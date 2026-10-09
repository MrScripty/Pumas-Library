//! Source-neutral, non-executing qualification of a bounded ONNX tensor graph.
//! Field numbers follow onnx/onnx v1.16.2 onnx.proto3. Unknown semantic fields
//! fail closed; this is a small structural reader, not a general ONNX checker.
use super::{acquired_package::Qualification, staging::VerifiedCopyInput, *};
use crate::{acquisition::ArtifactFile, model_library::types::FileFormat};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, SeekFrom},
};

const MODEL_LIMIT: u64 = 16 * 1024 * 1024;
const FIELD_LIMIT: usize = 100_000;
const ITEM_LIMIT: usize = 4096;

fn invalid(message: &str) -> PumasError {
    PumasError::Validation {
        field: "import.acquired_onnx".into(),
        message: message.into(),
    }
}

pub(super) fn qualify(inputs: &mut [VerifiedCopyInput], primary: usize) -> Result<Qualification> {
    // The existing single-file copy plan normalizes a basename. Preserve that
    // contract rather than claiming a nested layout it does not publish.
    if inputs.len() == 1 && inputs[primary].receipt.path.contains('/') {
        return Err(invalid(
            "A single embedded ONNX graph must use a root-level logical path",
        ));
    }
    let input = &mut inputs[primary];
    input.file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    (&mut input.file)
        .take(MODEL_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    input.file.seek(SeekFrom::Start(0))?;
    if bytes.len() as u64 > MODEL_LIMIT {
        return Err(invalid("ONNX graph exceeds 16 MiB limit"));
    }
    let budget = Cell::new(FIELD_LIMIT);
    let model = Message::parse(&bytes, &budget)?;
    model.only(&[1, 2, 3, 4, 5, 6, 7, 8])?;
    if !(7..=10).contains(&model.required(1)?.number()?) {
        return Err(invalid("Unsupported ONNX IR version"));
    }
    for tag in [2, 3, 4, 6] {
        model.optional_text(tag)?;
    }
    if let Some(field) = model.optional(5)? {
        field.number()?;
    }
    let opset = model.message(8)?;
    opset.only(&[1, 2])?;
    if opset
        .optional_text(1)?
        .is_some_and(|domain| !domain.is_empty())
        || opset.required(2)?.number()? != 13
    {
        return Err(invalid(
            "Only the standard ONNX opset 13 is supported by this structural class",
        ));
    }
    let graph = model.message(7)?;
    graph.only(&[1, 2, 5, 10, 11, 12, 13])?;
    if graph.required(2)?.text()?.is_empty() {
        return Err(invalid("ONNX graph name is required"));
    }
    graph.optional_text(10)?;
    let paths: BTreeMap<&str, u64> = inputs
        .iter()
        .map(|input| (input.receipt.path.as_str(), input.receipt.bytes))
        .collect();
    let primary_path = inputs[primary].receipt.path.as_str();
    let parent = primary_path
        .rsplit_once('/')
        .map(|(parent, _)| format!("{parent}/"))
        .unwrap_or_default();
    let mut referenced = BTreeSet::from([primary_path.to_owned()]);
    let mut values = BTreeMap::<String, Vec<u64>>::new();
    for field in graph.repeated(11)? {
        let (name, shape) = value_info(field.message(&budget)?)?;
        if values.insert(name, shape).is_some() {
            return Err(invalid("Duplicate ONNX graph input"));
        }
    }
    if values.is_empty() {
        return Err(invalid("This ONNX class requires a typed graph input"));
    }
    for field in graph.repeated(5)? {
        let tensor = field.message(&budget)?;
        tensor.only(&[1, 2, 8, 9, 12, 13, 14])?;
        tensor.optional_text(12)?;
        if tensor.required(2)?.number()? != 1 {
            return Err(invalid("Only FLOAT initializers are supported"));
        }
        let name = tensor.required(8)?.text()?;
        if name.is_empty() {
            return Err(invalid("ONNX tensor name is required"));
        }
        let mut shape = Vec::new();
        for field in tensor.repeated(1)? {
            match field {
                Field::Number(_, dim) => shape.push(*dim),
                Field::Bytes(_, packed) => {
                    let mut rest = *packed;
                    while !rest.is_empty() {
                        shape.push(varint(&mut rest)?);
                        if shape.len() > 16 {
                            return Err(invalid("ONNX tensor rank exceeds limit"));
                        }
                    }
                }
                _ => return Err(invalid("Invalid ONNX tensor dimensions")),
            }
        }
        let size = tensor_bytes(&shape)?;
        let location = tensor
            .optional(14)?
            .map(Field::number)
            .transpose()?
            .unwrap_or(0);
        match location {
            0 => {
                if !tensor.repeated(13)?.is_empty()
                    || tensor.required(9)?.bytes()?.len() as u64 != size
                {
                    return Err(invalid(
                        "Embedded ONNX tensor size or external-data state is invalid",
                    ));
                }
            }
            1 => {
                if tensor.optional(9)?.is_some() {
                    return Err(invalid("External ONNX tensor also contains embedded data"));
                }
                let mut entries = BTreeMap::new();
                for field in tensor.repeated(13)? {
                    let entry = field.message(&budget)?;
                    entry.only(&[1, 2])?;
                    let key = entry.required(1)?.text()?;
                    if !["location", "offset", "length"].contains(&key)
                        || entries.insert(key, entry.required(2)?.text()?).is_some()
                    {
                        return Err(invalid("Unsupported or duplicate ONNX external-data key"));
                    }
                }
                let relative = entries
                    .get("location")
                    .ok_or_else(|| invalid("Missing ONNX external-data location"))?;
                // Reuse manifest lexical admission. Resolve only against selected
                // logical paths, never an ambient path or a parser-supplied basepath.
                ArtifactFile::new(
                    *relative,
                    *relative,
                    None,
                    None,
                    crate::acquisition::FileVerificationRequirement::CompleteRepresentation,
                )
                .map_err(|_| invalid("Unsafe ONNX external-data location"))?;
                let resolved = format!("{parent}{relative}");
                if resolved == primary_path {
                    return Err(invalid("ONNX external data cannot alias its graph"));
                }
                let length = paths
                    .get(resolved.as_str())
                    .ok_or_else(|| invalid("Missing selected ONNX external-data file"))?;
                let decimal = |key: &str, default: u64| -> Result<u64> {
                    entries
                        .get(key)
                        .map(|value| {
                            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit())
                            {
                                return Err(invalid("Invalid ONNX external-data integer"));
                            }
                            value
                                .parse()
                                .map_err(|_| invalid("Overflowing ONNX external-data integer"))
                        })
                        .unwrap_or(Ok(default))
                };
                let offset = decimal("offset", 0)?;
                let extent = decimal(
                    "length",
                    length
                        .checked_sub(offset)
                        .ok_or_else(|| invalid("ONNX external-data offset exceeds file"))?,
                )?;
                if extent != size || offset.checked_add(extent).is_none_or(|end| end > *length) {
                    return Err(invalid(
                        "ONNX external-data range does not match tensor shape or selected file",
                    ));
                }
                referenced.insert(resolved);
            }
            _ => return Err(invalid("Invalid ONNX data_location")),
        }
        // This bounded class has immutable weights, not overridable input weights.
        if values.insert(name.to_owned(), shape).is_some() {
            return Err(invalid("Duplicate ONNX initializer or input name"));
        }
    }
    let nodes = graph.repeated(1)?;
    if nodes.is_empty() {
        return Err(invalid("This ONNX class requires a computation node"));
    }
    let mut node_names = BTreeSet::new();
    for field in nodes {
        let node = field.message(&budget)?;
        node.only(&[1, 2, 3, 4, 6, 7])?;
        node.optional_text(6)?;
        if node
            .optional_text(7)?
            .is_some_and(|domain| !domain.is_empty())
        {
            return Err(invalid("Custom ONNX operator domains are unsupported"));
        }
        if let Some(name) = node.optional_text(3)?.filter(|name| !name.is_empty()) {
            if !node_names.insert(name) {
                return Err(invalid("Duplicate ONNX node name"));
            }
        }
        let op = node.required(4)?.text()?;
        let arity = match op {
            "Identity" | "Relu" => 1,
            "Add" | "MatMul" => 2,
            _ => {
                return Err(invalid(
                    "Unsupported ONNX operator for this structural class",
                ))
            }
        };
        let args = node.repeated(1)?;
        if args.len() != arity {
            return Err(invalid("Invalid ONNX operator input arity"));
        }
        let shapes: Vec<&Vec<u64>> = args
            .iter()
            .map(|field| {
                values.get(field.text()?).ok_or_else(|| {
                    invalid("ONNX node input is absent or not topologically available")
                })
            })
            .collect::<Result<_>>()?;
        let shape = match op {
            "Add" if shapes[0] != shapes[1] => {
                return Err(invalid("This ONNX class requires equal-shape Add operands"))
            }
            "MatMul" => {
                if shapes[0].len() != 2 || shapes[1].len() != 2 || shapes[0][1] != shapes[1][0] {
                    return Err(invalid(
                        "This ONNX class requires compatible rank-two MatMul operands",
                    ));
                }
                vec![shapes[0][0], shapes[1][1]]
            }
            _ => shapes[0].clone(),
        };
        tensor_bytes(&shape)?;
        let output = node.required(2)?.text()?;
        if output.is_empty() || values.insert(output.to_owned(), shape).is_some() {
            return Err(invalid("Invalid or duplicate ONNX node output"));
        }
    }
    let outputs = graph.repeated(12)?;
    if outputs.is_empty() {
        return Err(invalid("ONNX graph requires a typed output"));
    }
    for tag in [12, 13] {
        let mut names = BTreeSet::new();
        for field in graph.repeated(tag)? {
            let (name, shape) = value_info(field.message(&budget)?)?;
            if !names.insert(name.clone()) || values.get(&name) != Some(&shape) {
                return Err(invalid(
                    "ONNX output/value information is duplicate, missing or has incompatible shape",
                ));
            }
        }
    }
    if referenced.len() != inputs.len()
        || inputs
            .iter()
            .any(|input| !referenced.contains(&input.receipt.path))
    {
        return Err(invalid("ONNX selection must contain exactly its graph and referenced external tensor files; companions/custom code are unsupported"));
    }
    Ok(Qualification {
        info: ModelTypeInfo {
            format: FileFormat::Onnx,
            model_type: ModelType::Unknown,
            family: None,
            extra: Default::default(),
        },
        diffusers: false,
        directory: inputs.len() > 1,
    })
}

fn tensor_bytes(shape: &[u64]) -> Result<u64> {
    if shape.len() > 16 || shape.iter().any(|dim| *dim == 0 || *dim > i64::MAX as u64) {
        return Err(invalid("Invalid or unsupported ONNX static shape"));
    }
    shape
        .iter()
        .try_fold(4_u64, |size, dim| size.checked_mul(*dim))
        .ok_or_else(|| invalid("ONNX tensor size overflows"))
}

fn value_info(value: Message<'_>) -> Result<(String, Vec<u64>)> {
    value.only(&[1, 2, 3])?;
    value.optional_text(3)?;
    let name = value.required(1)?.text()?;
    if name.is_empty() {
        return Err(invalid("Missing ONNX value name"));
    }
    let kind = value.message(2)?;
    kind.only(&[1])?;
    let tensor = kind.message(1)?;
    tensor.only(&[1, 2])?;
    if tensor.required(1)?.number()? != 1 {
        return Err(invalid("Only FLOAT graph values are supported"));
    }
    let shape = tensor.message(2)?;
    shape.only(&[1])?;
    let mut dims = Vec::new();
    for field in shape.repeated(1)? {
        let dim = field.message(value.1)?;
        dim.only(&[1])?;
        dims.push(dim.required(1)?.number()?);
    }
    tensor_bytes(&dims)?;
    Ok((name.to_owned(), dims))
}

#[derive(Clone, Copy)]
enum Field<'a> {
    Number(u32, u64),
    Bytes(u32, &'a [u8]),
    Fixed(u32),
}
impl<'a> Field<'a> {
    fn tag(&self) -> u32 {
        match self {
            Self::Number(tag, _) | Self::Bytes(tag, _) | Self::Fixed(tag) => *tag,
        }
    }
    fn number(&self) -> Result<u64> {
        if let Self::Number(_, number) = self {
            Ok(*number)
        } else {
            Err(invalid("Invalid ONNX protobuf wire type"))
        }
    }
    fn bytes(&self) -> Result<&'a [u8]> {
        if let Self::Bytes(_, bytes) = self {
            Ok(bytes)
        } else {
            Err(invalid("Invalid ONNX protobuf wire type"))
        }
    }
    fn text(&self) -> Result<&'a str> {
        std::str::from_utf8(self.bytes()?).map_err(|_| invalid("Invalid ONNX protobuf UTF-8"))
    }
    fn message(&self, budget: &'a Cell<usize>) -> Result<Message<'a>> {
        Message::parse(self.bytes()?, budget)
    }
}

struct Message<'a>(Vec<Field<'a>>, &'a Cell<usize>);
impl<'a> Message<'a> {
    fn parse(mut rest: &'a [u8], budget: &'a Cell<usize>) -> Result<Self> {
        let mut fields = Vec::new();
        while !rest.is_empty() {
            budget.set(
                budget
                    .get()
                    .checked_sub(1)
                    .ok_or_else(|| invalid("ONNX protobuf field limit exceeded"))?,
            );
            let key = varint(&mut rest)?;
            let tag = u32::try_from(key >> 3)
                .map_err(|_| invalid("Invalid ONNX protobuf field number"))?;
            if tag == 0 || tag > 536_870_911 {
                return Err(invalid("Invalid ONNX protobuf field number"));
            }
            let field = match key & 7 {
                0 => Field::Number(tag, varint(&mut rest)?),
                2 => {
                    let length = usize::try_from(varint(&mut rest)?)
                        .map_err(|_| invalid("ONNX protobuf length overflow"))?;
                    Field::Bytes(tag, take(&mut rest, length)?)
                }
                1 => {
                    take(&mut rest, 8)?;
                    Field::Fixed(tag)
                }
                5 => {
                    take(&mut rest, 4)?;
                    Field::Fixed(tag)
                }
                _ => return Err(invalid("Unsupported ONNX protobuf wire type")),
            };
            fields.push(field);
        }
        Ok(Self(fields, budget))
    }
    fn only(&self, tags: &[u32]) -> Result<()> {
        if self.0.iter().any(|field| !tags.contains(&field.tag())) {
            return Err(invalid("Unsupported ONNX semantic field (custom functions, attributes, metadata, sparse/training or future representation)"));
        }
        Ok(())
    }
    fn optional(&self, tag: u32) -> Result<Option<&Field<'a>>> {
        let mut fields = self.0.iter().filter(|field| field.tag() == tag);
        let first = fields.next();
        if fields.next().is_some() {
            return Err(invalid("Duplicate singular ONNX protobuf field"));
        }
        Ok(first)
    }
    fn required(&self, tag: u32) -> Result<&Field<'a>> {
        self.optional(tag)?
            .ok_or_else(|| invalid("Missing required ONNX protobuf field"))
    }
    fn optional_text(&self, tag: u32) -> Result<Option<&'a str>> {
        self.optional(tag)?.map(Field::text).transpose()
    }
    fn message(&self, tag: u32) -> Result<Message<'a>> {
        self.required(tag)?.message(self.1)
    }
    fn repeated(&self, tag: u32) -> Result<Vec<&Field<'a>>> {
        let fields: Vec<_> = self.0.iter().filter(|field| field.tag() == tag).collect();
        if fields.len() > ITEM_LIMIT {
            return Err(invalid("ONNX repeated item limit exceeded"));
        }
        Ok(fields)
    }
}

fn take<'a>(rest: &mut &'a [u8], length: usize) -> Result<&'a [u8]> {
    if length > rest.len() {
        return Err(invalid("Truncated ONNX protobuf field"));
    }
    let (head, tail) = rest.split_at(length);
    *rest = tail;
    Ok(head)
}
fn varint(rest: &mut &[u8]) -> Result<u64> {
    let mut result = 0_u64;
    for index in 0..10 {
        let byte = take(rest, 1)?[0];
        if index == 9 && byte > 1 {
            return Err(invalid("Overflowing ONNX protobuf varint"));
        }
        result |= ((byte & 0x7f) as u64) << (index * 7);
        if byte & 0x80 == 0 {
            return Ok(result);
        }
    }
    Err(invalid("Invalid ONNX protobuf varint"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_budget_is_shared_across_nested_messages() {
        let budget = Cell::new(3);
        // Outer field, two nested fields, then a further nested message.
        let model = Message::parse(&[10, 4, 10, 0, 16, 1], &budget).unwrap();
        let child = model.message(1).unwrap();
        assert_eq!(budget.get(), 0);
        assert!(child.message(1).is_ok()); // Empty message costs no fields.
        assert!(Message::parse(&[8, 1], &budget).is_err());
    }

    #[test]
    fn malformed_wire_lengths_tags_and_varints_fail_closed() {
        for bytes in [
            vec![0],
            vec![10, 2, 0],
            vec![8, 255],
            vec![8, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2],
            vec![11],
            vec![9, 0],
        ] {
            assert!(Message::parse(&bytes, &Cell::new(FIELD_LIMIT)).is_err());
        }
    }

    #[test]
    fn exact_field_item_and_shape_limits_fail_closed() {
        let bytes = [8, 1].repeat(FIELD_LIMIT);
        assert!(Message::parse(&bytes, &Cell::new(FIELD_LIMIT)).is_ok());
        assert!(Message::parse(&[bytes, vec![8, 1]].concat(), &Cell::new(FIELD_LIMIT)).is_err());
        let bytes = [8, 1].repeat(ITEM_LIMIT + 1);
        let budget = Cell::new(FIELD_LIMIT);
        assert!(Message::parse(&bytes, &budget)
            .unwrap()
            .repeated(1)
            .is_err());
        assert_eq!(tensor_bytes(&[1; 16]).unwrap(), 4);
        assert!(tensor_bytes(&[1; 17]).is_err());
        assert!(tensor_bytes(&[i64::MAX as u64, 2]).is_err());
    }

    #[test]
    fn singular_duplicates_and_fixed_wire_types_are_not_reinterpreted() {
        for bytes in [
            vec![8, 1, 8, 2],
            vec![9, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![13, 0, 0, 0, 0],
        ] {
            let budget = Cell::new(FIELD_LIMIT);
            let message = Message::parse(&bytes, &budget).unwrap();
            assert!(message.required(1).and_then(Field::number).is_err());
        }
    }
}
