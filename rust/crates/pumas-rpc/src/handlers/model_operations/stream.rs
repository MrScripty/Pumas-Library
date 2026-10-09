//! Bounded SSE decoding and typed projection, polled by the downstream body.
//! No producer task can outlive the original gateway body/transport custody.
use super::super::gateway_stream::GenerationCancellation;
use super::types::*;
use axum::{
    body::{Body, Bytes},
    http::header,
    response::{IntoResponse, Response},
};
use futures::{Stream, StreamExt};
use serde_json::Value;
use std::{convert::Infallible, pin::Pin};

type ProviderBody = Pin<Box<dyn Stream<Item = Result<Bytes, axum::Error>> + Send>>;
pub(super) struct Decoder {
    buffer: Vec<u8>,
    start: usize,
    scan: usize,
    data: String,
    data_lines: usize,
    event_bytes: usize,
    first_line: bool,
}
impl Decoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            start: 0,
            scan: 0,
            data: String::new(),
            data_lines: 0,
            event_bytes: 0,
            first_line: true,
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) {
        if self.start > 0 {
            self.buffer.drain(..self.start);
            self.scan = self.scan.saturating_sub(self.start);
            self.start = 0;
        }
        self.buffer.extend_from_slice(bytes);
    }
    pub fn frame(&mut self, eof: bool) -> Result<Option<String>, ErrorCode> {
        loop {
            let Some(index) = self.buffer[self.scan..]
                .iter()
                .position(|b| matches!(*b, b'\r' | b'\n'))
                .map(|offset| self.scan + offset)
            else {
                if self
                    .buffer
                    .len()
                    .saturating_sub(self.start)
                    .saturating_add(self.event_bytes)
                    > MAX_EVENT_BYTES
                {
                    return Err(ErrorCode::ResponseLimit);
                }
                if eof
                    && (self.buffer.len() != self.start
                        || self.data_lines != 0
                        || self.event_bytes != 0)
                {
                    return Err(ErrorCode::InvalidProviderResult);
                }
                self.scan = self.buffer.len();
                return Ok(None);
            };
            if index
                .saturating_sub(self.start)
                .saturating_add(self.event_bytes)
                > MAX_EVENT_BYTES
            {
                return Err(ErrorCode::ResponseLimit);
            }
            if self.buffer[index] == b'\r' && index + 1 == self.buffer.len() && !eof {
                self.scan = index;
                return Ok(None);
            }
            let end = index
                + if self.buffer[index] == b'\r' && self.buffer.get(index + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
            let mut line = std::str::from_utf8(&self.buffer[self.start..index])
                .map_err(|_| ErrorCode::InvalidProviderResult)?
                .to_owned();
            let line_bytes = end - self.start;
            self.start = end;
            self.scan = end;
            if self.first_line {
                line = line.strip_prefix('\u{feff}').unwrap_or(&line).to_owned();
                self.first_line = false;
            }
            if line.is_empty() {
                self.event_bytes = 0;
                if self.data_lines != 0 {
                    self.data_lines = 0;
                    return Ok(Some(std::mem::take(&mut self.data)));
                }
                continue;
            }
            self.event_bytes = self
                .event_bytes
                .checked_add(line_bytes)
                .ok_or(ErrorCode::ResponseLimit)?;
            if self.event_bytes > MAX_EVENT_BYTES {
                return Err(ErrorCode::ResponseLimit);
            }
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((&line, ""));
            if field == "data" {
                if self.data_lines != 0 {
                    self.data.push('\n');
                }
                self.data.push_str(value.strip_prefix(' ').unwrap_or(value));
                self.data_lines += 1;
            }
        }
    }
}
struct Projection {
    upstream: Option<ProviderBody>,
    cancellation: Option<GenerationCancellation>,
    decoder: Decoder,
    raw_bytes: usize,
    output_bytes: usize,
    request_id: String,
    capability: Capability,
    first: Option<StreamEvent>,
    finish: Option<FinishReason>,
    done: bool,
    eof: bool,
    terminal: bool,
}
impl Projection {
    fn payload(&mut self, data: String) -> Result<Option<StreamEvent>, ErrorCode> {
        if self.done {
            return Err(ErrorCode::InvalidProviderResult);
        }
        if data == "[DONE]" {
            if self.finish.is_none() {
                return Err(ErrorCode::InvalidProviderResult);
            }
            self.done = true;
            return Ok(None);
        }
        let value: Value =
            serde_json::from_str(&data).map_err(|_| ErrorCode::InvalidProviderResult)?;
        if value.get("error").is_some() {
            return Err(ErrorCode::ProviderFailure);
        }
        let choices = value
            .get("choices")
            .and_then(Value::as_array)
            .ok_or(ErrorCode::InvalidProviderResult)?;
        // Some adapters send a final usage-only envelope. It is never a result.
        if choices.is_empty() && value.get("usage").is_some_and(Value::is_object) {
            return Ok(None);
        }
        if choices.len() != 1 || choices[0].get("index").and_then(Value::as_u64) != Some(0) {
            return Err(ErrorCode::InvalidProviderResult);
        }
        let choice = &choices[0];
        let text = if self.capability == Capability::ChatGeneration {
            let delta = choice
                .get("delta")
                .and_then(Value::as_object)
                .ok_or(ErrorCode::InvalidProviderResult)?;
            if delta
                .keys()
                .any(|key| !matches!(key.as_str(), "role" | "content"))
                || delta
                    .get("role")
                    .is_some_and(|v| !v.is_null() && v.as_str() != Some("assistant"))
            {
                return Err(ErrorCode::InvalidProviderResult);
            }
            match delta.get("content") {
                None | Some(Value::Null) => "",
                Some(Value::String(text)) => text,
                _ => return Err(ErrorCode::InvalidProviderResult),
            }
        } else {
            choice
                .get("text")
                .and_then(Value::as_str)
                .ok_or(ErrorCode::InvalidProviderResult)?
        };
        if self.finish.is_some() {
            return Err(ErrorCode::InvalidProviderResult);
        }
        if let Some(reason) = choice.get("finish_reason").filter(|v| !v.is_null()) {
            self.finish = Some(
                serde_json::from_value(reason.clone())
                    .map_err(|_| ErrorCode::InvalidProviderResult)?,
            );
        }
        Ok((!text.is_empty()).then(|| StreamEvent::Delta {
            request_id: self.request_id.clone(),
            text: text.to_owned(),
        }))
    }
    async fn event(&mut self) -> Result<Option<StreamEvent>, ErrorCode> {
        if let Some(first) = self.first.take() {
            return Ok(Some(first));
        }
        loop {
            if let Some(frame) = self.decoder.frame(self.eof)? {
                if let Some(event) = self.payload(frame)? {
                    return Ok(Some(event));
                }
                continue;
            }
            if self.eof {
                if !self.done {
                    return Err(ErrorCode::InvalidProviderResult);
                }
                self.terminal = true;
                return Ok(Some(StreamEvent::Completed {
                    request_id: self.request_id.clone(),
                    finish_reason: self.finish.ok_or(ErrorCode::InvalidProviderResult)?,
                }));
            }
            match self
                .upstream
                .as_mut()
                .expect("active projection owns upstream")
                .next()
                .await
            {
                Some(Ok(bytes)) => {
                    self.raw_bytes = self
                        .raw_bytes
                        .checked_add(bytes.len())
                        .ok_or(ErrorCode::ResponseLimit)?;
                    if self.raw_bytes > MAX_BYTES {
                        return Err(ErrorCode::ResponseLimit);
                    }
                    self.decoder.feed(&bytes);
                }
                Some(Err(_)) => return Err(ErrorCode::TransportLost),
                None => self.eof = true,
            }
        }
    }
}
fn encode(event: &StreamEvent) -> Bytes {
    Bytes::from(format!(
        "event: {}\ndata: {}\n\n",
        event.name(),
        serde_json::to_string(event).expect("closed event is serializable")
    ))
}
pub fn response(
    response: Response,
    request: &OperationRequest,
    profile: &str,
    cancellation: Option<GenerationCancellation>,
) -> Response {
    let projection = Projection {
        upstream: Some(Box::pin(response.into_body().into_data_stream())),
        cancellation,
        decoder: Decoder::new(),
        raw_bytes: 0,
        output_bytes: 0,
        request_id: request.request_id.clone(),
        capability: request.capability,
        first: Some(StreamEvent::Started {
            contract_version: CONTRACT_VERSION,
            request_id: request.request_id.clone(),
            capability: request.capability,
            model: request.model.clone(),
            profile: profile.to_owned(),
        }),
        finish: None,
        done: false,
        eof: false,
        terminal: false,
    };
    let stream = futures::stream::unfold(projection, |mut state| async move {
        if state.terminal {
            return None;
        }
        let next = match state.cancellation.clone() {
            Some(mut cancellation) => {
                tokio::select! { biased; () = cancellation.cancelled() => Err(ErrorCode::TransportLost), event = state.event() => event }
            }
            None => state.event().await,
        };
        let event = match next {
            Ok(Some(event)) => event,
            Ok(None) => return None,
            Err(code) => {
                state.terminal = true;
                StreamEvent::Failed {
                    request_id: state.request_id.clone(),
                    error: OperationError {
                        code,
                        outcome: Outcome::Unknown,
                    },
                }
            }
        };
        let mut bytes = encode(&event);
        // Reserve room for a terminal failure even when progressive output reaches its bound.
        if bytes.len() > MAX_EVENT_BYTES
            || state.output_bytes.saturating_add(bytes.len()) > MAX_BYTES.saturating_sub(1024)
        {
            state.terminal = true;
            bytes = encode(&StreamEvent::Failed {
                request_id: state.request_id.clone(),
                error: OperationError {
                    code: ErrorCode::ResponseLimit,
                    outcome: Outcome::Unknown,
                },
            });
        }
        if state.terminal {
            state.upstream.take();
        }
        state.output_bytes += bytes.len();
        Some((Ok::<Bytes, Infallible>(bytes), state))
    });
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}
