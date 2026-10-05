//! Bounded ListObjectsV2 completion/selection guard; the SDK still decodes S3.
use aws_smithy_runtime_api::box_error::BoxError;
use aws_smithy_runtime_api::client::{
    interceptors::{context::AfterDeserializationInterceptorContextRef, Intercept},
    orchestrator::Metadata,
    runtime_components::RuntimeComponents,
};
use aws_smithy_types::config_bag::ConfigBag;

const MAX_XML_NODES: u32 = 4096;
const S3_NAMESPACE: &str = "http://s3.amazonaws.com/doc/2006-03-01/";

#[derive(Debug)]
pub(super) struct ListXmlGuard {
    pub(super) max_bytes: usize,
}

fn invalid() -> BoxError {
    std::io::Error::other("invalid or ambiguous listing evidence").into()
}

fn singleton<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    name: &str,
    required: bool,
) -> Result<Option<&'a str>, BoxError> {
    let mut nodes = parent
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == name);
    let Some(node) = nodes.next() else {
        return if required { Err(invalid()) } else { Ok(None) };
    };
    if nodes.next().is_some()
        || node.tag_name().namespace() != parent.tag_name().namespace()
        || node.children().any(|child| child.is_element())
    {
        return Err(invalid());
    }
    // roxmltree may coalesce adjacent Text/CDATA; callers also compare the value
    // with SDK output. Counting DOM text nodes alone cannot prove agreement.
    let mut text = node.children().filter(|child| child.is_text());
    let value = text.next().and_then(|child| child.text()).unwrap_or("");
    if text.next().is_some() {
        return Err(invalid());
    }
    Ok(Some(value))
}

impl Intercept for ListXmlGuard {
    fn name(&self) -> &'static str {
        "PumasListXmlGuard"
    }

    fn read_after_deserialization(
        &self,
        context: &AfterDeserializationInterceptorContextRef<'_>,
        _: &RuntimeComponents,
        cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        if cfg
            .load::<Metadata>()
            .is_none_or(|metadata| metadata.name() != "ListObjectsV2")
            || context.output_or_error().is_err()
        {
            return Ok(());
        }
        let bytes = context.response().body().bytes().ok_or_else(invalid)?;
        if bytes.len() > self.max_bytes {
            return Err(invalid());
        }
        let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
        let doc = roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions {
                allow_dtd: false,
                nodes_limit: MAX_XML_NODES,
                entity_resolver: None,
            },
        )
        .map_err(|_| invalid())?;
        let root = doc.root_element();
        if root.tag_name().name() != "ListBucketResult"
            || root
                .tag_name()
                .namespace()
                .is_some_and(|namespace| namespace != S3_NAMESPACE)
        {
            return Err(invalid());
        }
        let truncated = singleton(root, "IsTruncated", true)?;
        let token = singleton(root, "NextContinuationToken", false)?;
        match (truncated, token) {
            (Some("false"), None) => {}
            (Some("true"), Some(token)) if !token.is_empty() => {}
            _ => return Err(invalid()),
        }
        let output = context
            .output_or_error()
            .map_err(|_| invalid())?
            .downcast_ref::<aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output>()
            .ok_or_else(invalid)?;
        if output.is_truncated() != Some(truncated == Some("true"))
            || output.next_continuation_token() != token
        {
            return Err(invalid());
        }
        for (name, sdk_value) in [
            ("Name", output.name()),
            ("Prefix", output.prefix()),
            ("Delimiter", output.delimiter()),
            ("ContinuationToken", output.continuation_token()),
            ("StartAfter", output.start_after()),
            (
                "EncodingType",
                output.encoding_type().map(|value| value.as_str()),
            ),
        ] {
            if singleton(root, name, false)? != sdk_value {
                return Err(invalid());
            }
        }
        for (name, sdk_value) in [
            ("KeyCount", output.key_count()),
            ("MaxKeys", output.max_keys()),
        ] {
            if singleton(root, name, false)?
                .map(str::parse::<i32>)
                .transpose()
                .map_err(|_| invalid())?
                != sdk_value
            {
                return Err(invalid());
            }
        }
        let contents = root
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "Contents")
            .collect::<Vec<_>>();
        if contents.len() != output.contents().len() {
            return Err(invalid());
        }
        for (contents, sdk_value) in contents.into_iter().zip(output.contents()) {
            if contents.tag_name().namespace() != root.tag_name().namespace() {
                return Err(invalid());
            }
            if singleton(contents, "Key", true)? != sdk_value.key()
                || singleton(contents, "ETag", false)? != sdk_value.e_tag()
                || singleton(contents, "Size", false)?
                    .map(str::parse::<i64>)
                    .transpose()
                    .map_err(|_| invalid())?
                    != sdk_value.size()
            {
                return Err(invalid());
            }
        }
        let prefixes = root
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "CommonPrefixes")
            .collect::<Vec<_>>();
        if prefixes.len() != output.common_prefixes().len() {
            return Err(invalid());
        }
        for (prefix, sdk_value) in prefixes.into_iter().zip(output.common_prefixes()) {
            if prefix.tag_name().namespace() != root.tag_name().namespace()
                || singleton(prefix, "Prefix", true)? != sdk_value.prefix()
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
