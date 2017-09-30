#!/usr/bin/env ruby
require 'active_support/inflector'
require 'active_support/core_ext/string'

############################################################################
# This is a code generator to create Rust structs for all messages defined #
# by the second life protocol.                                             #
############################################################################

cur_dir = File.dirname(__FILE__)
TARGET_FILE = File.expand_path(File.join(cur_dir, "../src/all.rs"))
MESSAGE_TEMPLATE_FILE = File.expand_path(File.join(cur_dir, "./message_template.msg"))
PREAMBLE_FILE = File.expand_path(File.join(cur_dir, "./preamble.rs"))

require_relative './src/types.rb'
require_relative './src/parser.rb'

# Parse messages from template file.
messages = parse_messages(MESSAGE_TEMPLATE_FILE)

###################
# CODE GENERATION #
###################
puts "Found specifications for #{messages.size} messages.\n\n"

def generate_struct(message)
    # Generate block definitions,
    code = ""
    message.blocks.each do |block|
        code << "#[derive(Debug)]\n"
        code << "pub struct #{block.r_name} {\n"
        block.fields.each do |field|
            code << "\tpub #{field.r_name}: #{field.r_type},\n"
        end
        code << "}\n\n"
    end

    # Generate message definition.
    code << "#{message.comments}\n"
    code << "#[derive(Debug)]\n"
    code << "pub struct #{message.name} {\n"

    message.blocks.each do |block|
        if block.quantity.downcase == "single"
            code << "\tpub #{block.f_name}: #{block.r_name},\n"
        elsif block.quantity.downcase == "multiple"
            code << "\tpub #{block.f_name}: [#{block.r_name}; #{block.quantity_count}],\n"
        elsif block.quantity.downcase == "variable"
            code << "\tpub #{block.f_name}: Vec<#{block.r_name}>,\n"
        end
    end

    code << "}\n\n\n"
    code
end

def generate_message_types_enum(messages)
    code = ""
    code << "#[derive(Debug)]\n"
    code << "pub enum MessageInstance {\n"
    messages.each do |message|
        code << "\t#{message.name}(#{message.name}),\n"
    end
    code << "}\n\n"
    code << "impl MessageInstance {\n"
    code << "\tpub fn write_to<W: Write>(&self, buffer: &mut W) -> WriteMessageResult {\n"
    code << "\t\tmatch *self {\n"
    messages.each do |message|
        code << "\tMessageInstance::#{message.name}(ref msg) => msg.write_to(buffer),\n"
    end
    code << "\t\t}\n"
    code << "\t}\n"
    code << "}\n\n"

    messages.each do |message|
        code << "impl From<#{message.name}> for MessageInstance {\n"
        code << "\tfn from(msg: #{message.name}) -> Self {\n"
        code << "\t\tMessageInstance::#{message.name}(msg)\n"
        code << "\t}\n"
        code << "}\n\n"
    end

    code
end


#####################
# Generate writers. #
#####################

# Returns the byte array for a given message that can be written after the header
# to indicate the type of the message.
def generate_message_id_bytes(msg)
    if msg.frequency == "High"
        "[#{msg.id_byte 0}]"
    elsif msg.frequency == "Medium"
        "[0xff, #{msg.id_byte 1}]"
    elsif msg.frequency == "Low"
        "[0xff, 0xff, #{msg.id_byte 2}, #{msg.id_byte 3}]"
    elsif msg.frequency == "Fixed"
        "[0xff, 0xff, 0xff, #{msg.id_byte 3}]"
    else
        raise "Can't generate message type bytes for message: #{message}"
    end
end

# For a given field a single line writing the field once to a writer called `writer`.
# Since it is possible that we will enumerate over multiple sources in loops, the source
# object is provided here as an argument (of type string)
def generate_field_writer(field, source)
    value = "#{source}.#{field.r_name}"
    r_type = field.r_type
    if %w[u16 u32 u64 i16 i32 i64 f32 f64].include? r_type
        "buffer.write_#{r_type}::<LittleEndian>(#{value})?;\n"
    elsif %w[u8 i8].include? r_type
        "buffer.write_#{r_type}(#{value})?;\n"
    elsif r_type == "Uuid"
        "buffer.write(#{value}.as_bytes())?;\n"
    elsif r_type == "Ip4Addr"
        "buffer.write(&#{value}.octets())?;\n"
    elsif r_type == "IpPort"
        "buffer.write_u16::<LittleEndian>(#{value})?;\n"
    elsif r_type == "Vector3<f32>" or r_type == "Vector3<f64>"
        f_type = r_type[8..10]
        "buffer.write_#{f_type}::<LittleEndian>(#{value}.x)?;\n" +
        "buffer.write_#{f_type}::<LittleEndian>(#{value}.y)?;\n" +
        "buffer.write_#{f_type}::<LittleEndian>(#{value}.z)?;\n"
    elsif r_type == "Vector4<f32>"
        "buffer.write_f32::<LittleEndian>(#{value}.x)?;\n" +
        "buffer.write_f32::<LittleEndian>(#{value}.y)?;\n" +
        "buffer.write_f32::<LittleEndian>(#{value}.z)?;\n" +
        "buffer.write_f32::<LittleEndian>(#{value}.w)?;\n"
    elsif r_type == "Quaternion<f32>"
        "let normed_#{field.r_name} = UnitQuaternion::new(&#{value}).unwrap();\n" +
        "buffer.write_f32::<LittleEndian>(normed_#{field.r_name}.i)?;\n" +
        "buffer.write_f32::<LittleEndian>(normed_#{field.r_name}.j)?;\n" +
        "buffer.write_f32::<LittleEndian>(normed_#{field.r_name}.k)?;\n"
    elsif r_type == "bool"
        "buffer.write_u8(#{value} as u8)?;\n"
    elsif r_type == "Vec<u8>"
        "buffer.write(&#{value}[..])?;\n"
    elsif r_type[0...4] == "[u8;"
        "buffer.write(&#{value})?;\n"
    else
        raise "No rule for field writer generation of field: #{field}"
    end
end

def generate_field_reader(field)
    r_type = field.r_type
    if %w[u16 u32 u64 i16 i32 i64 f32 f64].include? r_type
        "buffer.read_#{r_type}::<LittleEndian>()?"
    elsif %w[u8 i8].include? r_type
        "buffer.read_#{r_type}()?"
    elsif r_type == "Uuid"
        "{ let mut raw = [0; 4]; buffer.read_exact(&mut raw)?; Uuid::from_bytes(&raw)? }"
    elsif r_type == "Ip4Addr"
        "{ let mut raw = [0; 4]; buffer.read_exact(&mut raw)?; Ip4Addr::from(raw) }"
    elsif r_type == "IpPort"
        "buffer.read_u16::<LittleEndian>()?"
    elsif r_type == "Vector3<f32>" or r_type == "Vector3<f64>"
        f_type = r_type[8..10]
        "Vector3::new(buffer.read_#{f_type}::<LittleEndian>()?,
                      buffer.read_#{f_type}::<LittleEndian>()?,
                      buffer.read_#{f_type}::<LittleEndian>()?)"
    elsif r_type == "Vector4<f32>"
        "Vector4::new(buffer.read_f32::<LittleEndian>()?,
                      buffer.read_f32::<LittleEndian>()?,
                      buffer.read_f32::<LittleEndian>()?,
                      buffer.read_f32::<LittleEndian>()?)"
    elsif r_type == "Quaternion<f32>"
        "Quaternion::from_parts(1., Vector3::new(
            buffer.read_f32::<LittleEndian>()?,
            buffer.read_f32::<LittleEndian>()?,
            buffer.read_f32::<LittleEndian>()?
        ))"
    elsif r_type == "bool"
        "buffer.read_u8()? == 1"
    elsif r_type == "Vec<u8>"
        "{ let n = buffer.read_u8()? as usize; let mut raw = vec![0; n]; buffer.read_exact(&mut raw)?; raw }"
    elsif r_type[0...4] == "[u8;"
        "{ let mut raw = [0; #{field.count}]; buffer.read_exact(&mut raw)?; raw }"
    else
        raise "No rule for field reader generation of field: #{field}"
    end
end

def generate_block_reader_impl(block)
    out = ""
    out << "impl #{block.r_name} {\n"
    out << "\tfn read_from<R: ?Sized>(buffer: &mut R) -> Result<Self, ReadMessageError> where R: Read {\n"
    out << "\t\tOk(#{block.r_name} {\n"
    block.fields.each do |field|
        out << "\t\t\t#{field.r_name}: #{generate_field_reader(field)},\n"
    end
    out << "\t\t})\n"
    out << "\t}\n"
    out << "}\n\n"
    out
end

def generate_message_impl(message)
    out = ""
    out << "impl Message for #{message.name} {\n"

    # Writer
    #########
    out << "\tfn write_to<W: ?Sized>(&self, buffer: &mut W) -> WriteMessageResult where W: Write {\n"
    out << "\t\t// Write the message number.\n"
    out << "\t\tbuffer.write(&#{generate_message_id_bytes(message)})?;\n"
    message.blocks.each do |block|
        out << "\t\t// Block #{block.ll_name}\n"
        if block.quantity == "Single"
            block.fields.each do |field|
                out << "\t\t" + generate_field_writer(field, "self.#{block.f_name}")
            end
        elsif block.quantity == "Multiple"
            out << "\t\tfor i in 0..#{block.quantity_count} {\n"
            block.fields.each do |field|
                out << "\t\t\t" + generate_field_writer(field, "self.#{block.f_name}[i]")
            end
            out << "\t\t}\n"
        elsif block.quantity == "Variable"
            out << "\t\tbuffer.write_u8(self.#{block.f_name}.len() as u8)?;\n"
            out << "\t\tfor item in &self.#{block.f_name} {\n"
            block.fields.each do |field|
                out << "\t\t\t" + generate_field_writer(field, "item")
            end
            out << "\t\t}\n"
        else
            raise "Invalid block quantity: #{block.quantity}"
        end
    end
    out << "\tOk(())\n"
    out << "\t}\n\n"

    # Reader
    #########
    buffer_var = (message.blocks.map{|block| block.fields.count}.inject(:+).to_i != 0) ? "buffer" : "_"
    out << "\tfn read_from<R: ?Sized>(#{buffer_var}: &mut R) -> Result<MessageInstance, ReadMessageError> where R: Read {\n"
    message.blocks.each do |block|
        out << "\t\t// Block #{block.ll_name}\n"
        if block.quantity == "Single"
            out << "\t\tlet #{block.f_name} = #{block.r_name}::read_from(buffer)?;\n"
        elsif block.quantity == "Multiple"
            out << "\t\tlet #{block.f_name} = [\n"
            block.quantity_count.to_i.times do
                out << "\t\t\t#{block.r_name}::read_from(buffer)?,\n"
            end
            out << "\t\t];\n"
        elsif block.quantity == "Variable"
            count_var = "_#{block.f_name}_count"
            out << "\t\tlet mut #{block.f_name} = Vec::new();\n"
            out << "\t\tlet #{count_var} = buffer.read_u8()?;\n"
            out << "\t\tfor _ in 0..#{count_var} {\n"
            out << "\t\t\t#{block.f_name}.push(#{block.r_name}::read_from(buffer)?);\n"
            out << "\t\t}\n"
        else
            puts "Read implementation for blocks with quantity '#{block.quantity}' not implemented yet."
            return ""
        end
    end
    out << "\t\tOk(MessageInstance::#{message.name}(#{message.name} {\n"
    message.blocks.each do |block|
        out << "\t\t\t#{block.f_name}: #{block.f_name},\n"
    end
    out << "\t\t}))\n"
    out << "\t}\n"
    out << "}\n\n"

    out
end



# Generates the read_message function that handles all possible message types.
def generate_read_func(messages)
    out = ""
    out << "pub fn read_message<R: ?Sized>(buffer: &mut R, message_num: u32) -> Result<MessageInstance, ReadMessageError> where R: Read {\n"
    out << "\tmatch message_num {\n"

    messages.each do |message|
        out << "\t\t#{message.message_num} => return #{message.name}::read_from(buffer),\n"
    end

    out << "\t\t_ => return Err(ReadMessageError::UnknownMessageNumber(message_num))\n"

    out << "\t}\n"
    out << "}\n\n"
    out
end

# generate messages module.
File.open(TARGET_FILE, "w") do |file|
    file.write <<-INFO.strip_heredoc
        #![allow(non_snake_case)]
        #![allow(non_camel_case_types)]

        ///
        /// THIS FILE WAS AUTOGENERATED.
        /// DON'T EDIT MANUALLY!
        /// If you want to change the file, edit the generator script `generate/generate.rb`.
        ///

    INFO
    file.write File.read(PREAMBLE_FILE)
    
    file.write generate_read_func(messages)
    
    messages.each { |msg| file.write generate_struct(msg) }
    file.write generate_message_types_enum(messages)
    file.write "\n\n// Block IMPLEMENTATIONS\n\n\n\n"
    messages.each do |message|
        message.blocks.each do |block|
            file.write generate_block_reader_impl(block)
        end
    end

    file.write "\n\n// Message IMPLEMENTATIONS\n\n\n\n"
    messages.each do |msg|
        code = generate_message_impl(msg)
        file.write code unless code.empty?
    end
end

if system 'which rustfmt'
    dir = File.absolute_path File.dirname(__FILE__)
    system "rustfmt --write-mode overwrite --config-path '#{dir}' '#{TARGET_FILE}'"
else
    puts "Warning: rustfmt not installed, please install and rerun!"
end

