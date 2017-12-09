def generate_struct(message):
    # Generate block definitions.
    code = ""
    for block in message.blocks:
        code += "#[derive(Clone, Debug)]\n"
        code += "pub struct %s {\n" % block.r_name
        for field in block.fields:
            code += "\t%s\n" % field.doc
            code += "\tpub %s: %s,\n" % (field.r_name, field.r_type)
        code += "}\n\n"

    # Generate message definition.
    code += "%s\n" % message.doc
    code += "#[derive(Clone, Debug)]\n"
    code += "pub struct %s {\n" % message.name

    for block in message.blocks:
        q = block.quantity.lower()
        if q == "single":
            code += "\tpub %s: %s,\n" % (block.f_name, block.r_name)
        elif q == "multiple":
            code += "\tpub %s: ArrayVec<[%s; %s]>,\n" % (block.f_name, block.r_name, block.quantity_count)
        elif q == "variable":
            code += "\tpub %s: Vec<%s>,\n" % (block.f_name, block.r_name)
    code += "}\n"
    code += "\n\n"
    return code

def generate_message_id_bytes(msg):
    """
    Returns the byte array for a given message that can be written directly after the header to indicate the
    type of message.
    """
    if msg.frequency_class == "high":
        return "[%s]" % msg.id_byte(0)
    elif msg.frequency_class == "medium":
        return "[0xff, %s]" % msg.id_byte(1)
    elif msg.frequency_class == "low":
        return "[0xff, 0xff, %s, %s]" % (msg.id_byte(2), msg.id_byte(3))
    elif msg.frequency_class == "fixed":
        return "[0xff, 0xff, 0xff, %s]" % msg.id_byte(3)
    else:
        raise RuntimeError("Can't generate message type bytes for message: %s" % msg.__dict__)

def generate_field_writer(field, source):
    """ Generate code that writes the field from the source binding to the output buffer. """
    value = source + "." + field.r_name
    r_type = field.r_type

    if r_type in "u16 u32 u64 i16 i32 i64 f32 f64":
        return "buffer.write_%s::<LittleEndian>(%s)?;\n" % (r_type, value)
    elif r_type in "u8 i8":
        return "buffer.write_%s(%s)?;\n" % (r_type, value)
    elif r_type == "Uuid":
        return "buffer.write(%s.as_bytes())?;\n" % value
    elif r_type == "Ip4Addr":
        return "buffer.write(&%s.octets())?;\n" % value
    elif r_type == "IpPort":
        return "buffer.write_u16::<LittleEndian>(%s)?;\n" % value
    elif r_type in "Vector3<f32> Vector3<f64>":
        f_type = r_type[8:11]
        return """buffer.write_{0}::<LittleEndian>({1}.x)?;\n
                  buffer.write_{0}::<LittleEndian>({1}.y)?;\n
                  buffer.write_{0}::<LittleEndian>({1}.z)?;\n""".format(f_type, value)
    elif r_type == "Vector4<f32>":
        return """buffer.write_f32::<LittleEndian>({0}.x)?;\n
                  buffer.write_f32::<LittleEndian>({0}.y)?;\n
                  buffer.write_f32::<LittleEndian>({0}.z)?;\n
                  buffer.write_f32::<LittleEndian>({0}.w)?;\n""".format(value)
    elif r_type == "Quaternion<f32>":
        return """let normed_{0} = UnitQuaternion::from_quaternion({1}).unwrap();\n
                  buffer.write_f32::<LittleEndian>(normed_{0}.i)?;\n
                  buffer.write_f32::<LittleEndian>(normed_{0}.j)?;\n
                  buffer.write_f32::<LittleEndian>(normed_{0}.k)?;\n""".format(field.r_name, value)
    elif r_type == "bool":
        return "buffer.write_u8(%s as u8)?;\n" % value
    elif r_type == "Vec<u8>":
        if field.count == "1":
            out = "buffer.write_u8(%s.len() as u8)?;\n" % value
        elif field.count == "2":
            out = "buffer.write_u16::<LittleEndian>(%s.len() as u16)?;\n" % value
        else:
            raise RuntimeError("Invalid count for field: {}", field.__dict__)
        out += "buffer.write(&%s[..])?;\n" % value
        return out
    elif r_type[0:4] == "[u8;":
        return "buffer.write(&%s)?;\n" % value
    else:
        raise RuntimeError("No rule for field writer generation of r_type: %s" % field.r_type)

def generate_field_reader(field):
    r_type = field.r_type
    if r_type in "u16 u32 u64 i16 i32 i64 f32 f64":
        return "buffer.read_%s::<LittleEndian>()?" % r_type
    elif r_type in "u8 i8":
        return "buffer.read_%s()?" % r_type
    elif r_type == "Uuid":
        return "{ let mut raw = [0u8; 16]; buffer.read_exact(&mut raw)?; Uuid::from_bytes(&raw)? }"
    elif r_type == "Ip4Addr":
        return "{ let mut raw = [0u8; 4]; buffer.read_exact(&mut raw)?; Ip4Addr::from(raw) }"
    elif r_type == "IpPort":
        return "buffer.read_u16::<LittleEndian>()?"
    elif r_type in "Vector3<f32> Vector3<f64>":
        f_type = r_type[8:11]
        return """Vector3::new(buffer.read_{0}::<LittleEndian>()?,
                               buffer.read_{0}::<LittleEndian>()?,
                               buffer.read_{0}::<LittleEndian>()?)""".format(f_type)
    elif r_type == "Vector4<f32>":
        return """Vector4::new(buffer.read_f32::<LittleEndian>()?,
                               buffer.read_f32::<LittleEndian>()?,
                               buffer.read_f32::<LittleEndian>()?,
                               buffer.read_f32::<LittleEndian>()?)"""
    elif r_type == "Quaternion<f32>":
        # TODO: Verify if this is doing the right thing.
        return """Quaternion::from_parts(1., Vector3::new(
                      buffer.read_f32::<LittleEndian>()?,
                      buffer.read_f32::<LittleEndian>()?,
                      buffer.read_f32::<LittleEndian>()?))"""
    elif r_type == "bool":
        return "buffer.read_u8()? == 1"
    elif r_type == "Vec<u8>":
        if field.count == "1":
            out = "{\n\tlet n = buffer.read_u8()? as usize;\n"
        elif field.count == "2":
            out = "{\n\tlet n = buffer.read_u16::<LittleEndian>()? as usize;\n"
        else:
            raise RuntimeError("invalid quantity for field: %s" % field.__dict__)
        out += "\tlet mut raw = vec![0; n]; buffer.read_exact(&mut raw)?; raw }"
        return out
    elif r_type[0:4] == "[u8;":
        return "{ let mut raw = [0; %s]; buffer.read_exact(&mut raw)?; raw }" % field.count
    else:
        raise RuntimeError("No rule for field reader generation of r_type: %s" % field)

def generate_block_reader_impl(block):
    out = ""
    out += "impl %s {\n" % block.r_name
    out += "\tfn read_from<R: ?Sized>(buffer: &mut R) -> Result<Self, ReadError> where R: Read {\n"
    out += "\t\tOk(%s {\n" % block.r_name
    for field in block.fields:
        out += "\t\t\t%s: %s,\n" % (field.r_name, generate_field_reader(field))
    out += "\t\t})\n"
    out += "\t}\n"
    out += "}\n\n"
    return out

def generate_message_impl(message):
    out = ""
    out += "impl Message for %s {\n" % message.name

    ##############
    #   Writer   #
    ##############

    out += "\tfn write_to<W: ?Sized>(&self, buffer: &mut W) -> WriteMessageResult where W: Write {\n"
    out += "\t\t// Write the message number.\n"
    out += "\t\tbuffer.write(&%s)?;\n" % generate_message_id_bytes(message)
    for block in message.blocks:
        out += "\t\t// Block %s\n" % block.ll_name
        if block.quantity == "single":
            for field in block.fields:
                out += "\t\t" + generate_field_writer(field, "self.%s" % block.f_name)
        elif block.quantity == "multiple":
            out += "\t\tfor i in 0..%s {\n" % block.quantity_count
            for field in block.fields:
                out += "\t\t\t" + generate_field_writer(field, "self.%s[i]" % block.f_name)
            out += "\t\t}\n"
        elif block.quantity == "variable":
            out += "\t\tbuffer.write_u8(self.%s.len() as u8)?;\n" % block.f_name
            out += "\t\tfor item in &self.%s {\n" % block.f_name
            for field in block.fields:
                out += "\t\t\t" + generate_field_writer(field, "item")
            out += "\t\t}\n"
        else:
            raise RuntimeError("Invalid block quantity: %s" % block.quantity)
    out += "\t\tOk(())\n"
    out += "\t}\n\n"

    ##############
    #   Reader   #
    ##############
    if sum(map(lambda b: len(b.fields), message.blocks)) > 0:
        buffer_var = "buffer"
    else:
        buffer_var = "_"

    out += "\tfn read_from<R: ?Sized>(%s: &mut R) -> Result<MessageInstance, ReadError> where R: Read {\n" % buffer_var
    for block in message.blocks:
        out += "\t\t// Block %s\n" % block.ll_name
        if block.quantity == "single":
            out += "\t\tlet %s = %s::read_from(buffer)?;\n" % (block.f_name, block.r_name)
        elif block.quantity == "multiple":
            out += "\t\tlet %s = ArrayVec::from([\n" % block.f_name
            for _ in range(block.quantity_count):
                out += "\t\t\t%s::read_from(buffer)?,\n" % block.r_name
            out += "\t\t]);\n"
        elif block.quantity == "variable":
            # FIXME TODO handle Variable 1, Variable 2 separately.
            count_var = "_%s_count" % block.f_name
            out += "\t\tlet mut %s = Vec::new();\n" % block.f_name
            out += "\t\tlet %s = buffer.read_u8()?;\n" % count_var
            out += "\t\tfor _ in 0..%s {\n" % count_var
            out += "\t\t\t%s.push(%s::read_from(buffer)?);\n" % (block.f_name, block.r_name)
            out += "\t\t}\n"
        else:
            raise RuntimeError("Read implementation for blocks with quantity '%s' not implemented yet." % block.quantity)

    out += "\t\tOk(MessageInstance::%s(%s {\n" % (message.name, message.name)
    for block in message.blocks:
        out += "\t\t\t%s: %s,\n" % (block.f_name, block.f_name)
    out += "\t\t}))\n"
    out += "\t}\n"
    out += "}\n\n"

    return out

def generate_message_type_enum(all_msgnames):
    code = ""
    code += "#[derive(Clone, Debug, Eq, Hash, PartialEq)]\n"
    code += "pub enum MessageType {\n"
    for name in all_msgnames:
        code += "\t%s,\n" % name
    code += "}\n\n"
    return code

def generate_message_instance_enum(all_msgnames, messages):
    code = ""
    code += "#[derive(Clone, Debug)]\n"
    code += "pub enum MessageInstance {\n"
    for name in all_msgnames:
        code += "\t%s(%s),\n" % (name, name)
    code += "}\n\n"

    code += "impl MessageInstance {\n"

    # MessageInstance::message_type
    code += "\tpub fn message_type(&self) -> MessageType {\n"
    code += "\t\tmatch *self {\n"
    for name in all_msgnames:
        code += "\t\t\tMessageInstance::%s(_) => MessageType::%s,\n" % (name, name)
    code += "\t\t}\n"
    code += "\t}\n\n"

    # MessageInstance::write_to
    code += "\tpub fn write_to<W: Write>(&self, buffer: &mut W) -> WriteMessageResult {\n"
    code += "\t\tmatch *self {\n"
    for name in all_msgnames:
        code += "\t\t\tMessageInstance::%s(ref msg) => msg.write_to(buffer),\n" % name
    code += "\t\t}\n"
    code += "\t}\n"

    # MessageInstance::read_message
    code += "\tpub fn read_message<R: ?Sized>(buffer: &mut R, message_num: u32) -> Result<MessageInstance, ReadError> where R: Read {\n"
    code += "\t\tmatch message_num {\n"
    for message in messages:
        code += "\t\t\t%s => %s::read_from(buffer),\n" % (message.message_num, message.name)
    code += "\t\t\t_ => Err(ReadErrorKind::UnknownMessageNumber(message_num).into())\n"
    code += "\t\t}\n"
    code += "\t}\n"

    code += "}\n\n"

    for name in all_msgnames:
        code += "impl From<%s> for MessageInstance {\n" % name
        code += "\tfn from(msg: %s) -> Self {\n" % name
        code += "\t\tMessageInstance::%s(msg)\n" % name
        code += "\t}\n"
        code += "}\n\n"

    return code
