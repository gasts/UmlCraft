//! Writes a [`Graph`] as a serialization stream, the way `ObjectOutputStream` writes it.
//! Tooling for checking the reader and building test files; the import does not use it.
//!
//! Objects are written where the traversal of the roots first meets them and referenced by handle
//! afterwards, so the handles of a graph that was read come out as they were.

use std::collections::HashMap;

use super::*;

/// Writes `graph` as a complete stream. `extra_header` is what a custom `writeStreamHeader`
/// writes after the standard magic and version.
///
/// The writer recurses along the graph, so deep graphs need a thread with a large stack.
pub fn write(graph: &Graph, extra_header: &[u8]) -> Result<Vec<u8>> {
    let mut w = Writer {
        graph,
        out: Vec::new(),
        objects: HashMap::new(),
        classes: HashMap::new(),
        next: BASE_HANDLE,
        depth: 0,
    };
    w.u16(MAGIC);
    w.u16(VERSION);
    w.out.extend_from_slice(extra_header);
    for content in &graph.roots {
        w.content(content)?;
    }
    Ok(w.out)
}

struct Writer<'g> {
    graph: &'g Graph,
    out: Vec<u8>,
    objects: HashMap<ObjId, u32>,
    classes: HashMap<ClassId, u32>,
    next: u32,
    depth: usize,
}

impl Writer<'_> {
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }

    fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_be_bytes());
    }

    fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_be_bytes());
    }

    fn i64(&mut self, v: i64) {
        self.out.extend_from_slice(&v.to_be_bytes());
    }

    fn utf(&mut self, s: &str) -> Result<()> {
        let bytes = encode_modified_utf8(s);
        let len = u16::try_from(bytes.len()).map_err(|_| StreamError::Unwritable("long name"))?;
        self.u16(len);
        self.out.extend_from_slice(&bytes);
        Ok(())
    }

    fn handle(&mut self) -> u32 {
        let handle = self.next;
        self.next += 1;
        handle
    }

    fn reference(&mut self, handle: u32) {
        self.u8(TC_REFERENCE);
        self.i32(handle as i32);
    }

    fn content(&mut self, content: &Content) -> Result<()> {
        match content {
            Content::Block(bytes) => {
                if let Ok(len) = u8::try_from(bytes.len()) {
                    self.u8(TC_BLOCKDATA);
                    self.u8(len);
                } else {
                    self.u8(TC_BLOCKDATALONG);
                    self.i32(bytes.len() as i32);
                }
                self.out.extend_from_slice(bytes);
            }
            Content::Value(value) => self.value(value)?,
            Content::Reset => {
                self.u8(TC_RESET);
                self.objects.clear();
                self.classes.clear();
                self.next = BASE_HANDLE;
            }
        }
        Ok(())
    }

    /// Contents followed by `TC_ENDBLOCKDATA`.
    fn annotation(&mut self, items: &[Content]) -> Result<()> {
        for item in items {
            self.content(item)?;
        }
        self.u8(TC_ENDBLOCKDATA);
        Ok(())
    }

    fn value(&mut self, value: &Value) -> Result<()> {
        match *value {
            Value::Null => {
                self.u8(TC_NULL);
                Ok(())
            }
            Value::Object(id) => {
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return Err(StreamError::TooDeep);
                }
                let result = self.object(id);
                self.depth -= 1;
                result
            }
            _ => Err(StreamError::Unwritable("primitive value where an object belongs")),
        }
    }

    fn object(&mut self, id: ObjId) -> Result<()> {
        if let Some(&handle) = self.objects.get(&id) {
            self.reference(handle);
            return Ok(());
        }
        let graph = self.graph;
        match graph.object(id) {
            Object::String(s) => {
                let bytes = encode_modified_utf8(s);
                if let Ok(len) = u16::try_from(bytes.len()) {
                    self.u8(TC_STRING);
                    self.u16(len);
                } else {
                    self.u8(TC_LONGSTRING);
                    self.i64(bytes.len() as i64);
                }
                let handle = self.handle();
                self.objects.insert(id, handle);
                self.out.extend_from_slice(&bytes);
            }
            Object::Instance { class, data } => {
                self.u8(TC_OBJECT);
                self.class_desc(Some(*class))?;
                let handle = self.handle();
                self.objects.insert(id, handle);
                let mut hierarchy = Vec::new();
                let mut current = Some(*class);
                while let Some(c) = current {
                    hierarchy.push(c);
                    current = graph.class(c).super_class;
                }
                if hierarchy.len() != data.len() {
                    return Err(StreamError::Unwritable("class data that does not fit the class"));
                }
                for (&c, class_data) in hierarchy.iter().rev().zip(data) {
                    self.class_data(graph.class(c), class_data)?;
                }
            }
            Object::Array { class, items } => {
                self.u8(TC_ARRAY);
                self.class_desc(Some(*class))?;
                let handle = self.handle();
                self.objects.insert(id, handle);
                let element = super::array_element(&graph.class(*class).name)?;
                self.i32(items.len() as i32);
                for item in items {
                    self.typed(element, item)?;
                }
            }
            Object::Enum { class, constant, name } => {
                self.u8(TC_ENUM);
                self.class_desc(Some(*class))?;
                let handle = self.handle();
                self.objects.insert(id, handle);
                // The constant's name is always written as a new string.
                let bytes = encode_modified_utf8(constant);
                let len = u16::try_from(bytes.len())
                    .map_err(|_| StreamError::Unwritable("long enum constant"))?;
                self.u8(TC_STRING);
                self.u16(len);
                let handle = self.handle();
                if let Value::Object(name) = name {
                    self.objects.insert(*name, handle);
                }
                self.out.extend_from_slice(&bytes);
            }
            Object::Class(class) => {
                self.u8(TC_CLASS);
                self.class_desc(Some(*class))?;
                let handle = self.handle();
                self.objects.insert(id, handle);
            }
        }
        Ok(())
    }

    fn class_data(&mut self, desc: &ClassDesc, data: &ClassData) -> Result<()> {
        if desc.flags & SC_EXTERNALIZABLE != 0 {
            if desc.flags & SC_BLOCK_DATA == 0 {
                return Err(StreamError::Externalizable(desc.name.to_string()));
            }
            return self.annotation(&data.annotation);
        }
        if desc.flags & SC_SERIALIZABLE == 0 {
            return Ok(());
        }
        for (i, field) in desc.fields.iter().enumerate() {
            // Fields are matched by name, so data built by hand need not list every field; a
            // field that is missing is written as zero or null.
            let value = match data.fields.get(i) {
                Some((name, value)) if *name == field.name => Some(value),
                _ => data.fields.iter().find(|(name, _)| *name == field.name).map(|(_, v)| v),
            };
            self.typed(field.ty(), value.unwrap_or(&Value::Null))?;
        }
        if desc.flags & SC_WRITE_METHOD != 0 {
            self.annotation(&data.annotation)?;
        }
        Ok(())
    }

    fn typed(&mut self, ty: FieldType, value: &Value) -> Result<()> {
        let int = || value.as_i64().unwrap_or(0);
        let float = || value.as_f64().unwrap_or(0.0);
        match ty {
            FieldType::Byte => self.u8(int() as u8),
            FieldType::Char => self.u16(int() as u16),
            FieldType::Double => self.i64(float().to_bits() as i64),
            FieldType::Float => self.i32((float() as f32).to_bits() as i32),
            FieldType::Int => self.i32(int() as i32),
            FieldType::Long => self.i64(int()),
            FieldType::Short => self.u16(int() as u16),
            FieldType::Bool => self.u8(value.as_bool().unwrap_or(int() != 0) as u8),
            FieldType::Object => self.value(value)?,
        }
        Ok(())
    }

    fn class_desc(&mut self, class: Option<ClassId>) -> Result<()> {
        let Some(class) = class else {
            self.u8(TC_NULL);
            return Ok(());
        };
        if let Some(&handle) = self.classes.get(&class) {
            self.reference(handle);
            return Ok(());
        }
        let desc = self.graph.class(class);
        if let Some(interfaces) = &desc.interfaces {
            self.u8(TC_PROXYCLASSDESC);
            let handle = self.handle();
            self.classes.insert(class, handle);
            self.i32(interfaces.len() as i32);
            for name in interfaces {
                self.utf(name)?;
            }
        } else {
            self.u8(TC_CLASSDESC);
            self.utf(&desc.name)?;
            self.i64(desc.serial_version_uid);
            let handle = self.handle();
            self.classes.insert(class, handle);
            self.u8(desc.flags);
            let count = u16::try_from(desc.fields.len())
                .map_err(|_| StreamError::Unwritable("too many fields"))?;
            self.u16(count);
            for field in &desc.fields {
                self.u8(field.code);
                self.utf(&field.name)?;
                if let Some(type_name) = &field.type_name {
                    self.value(type_name)?;
                }
            }
        }
        self.annotation(&desc.annotation)?;
        self.class_desc(desc.super_class)
    }
}

/// Java's "modified UTF-8": NUL as two bytes and supplementary characters as surrogate pairs.
pub(crate) fn encode_modified_utf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for unit in s.encode_utf16() {
        match unit {
            0x0001..=0x007F => out.push(unit as u8),
            0x0000 | 0x0080..=0x07FF => {
                out.push(0xC0 | (unit >> 6) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (unit >> 12) as u8);
                out.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_utf8_round_trip() {
        for s in ["abc", "\0", "Größe", "😀", ""] {
            assert_eq!(decode_modified_utf8(&encode_modified_utf8(s)), s);
        }
        assert_eq!(encode_modified_utf8("\0"), [0xC0, 0x80]);
        assert_eq!(encode_modified_utf8("😀"), [0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }
}
