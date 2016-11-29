#!/usr/bin/env ruby
require 'ostruct'
require 'active_support/inflector'

# This is a code generator to create Rust structs for all messages defined
# by the second life protocol.

TARGET_FILE="./src/messages.rs"

#########################################
# PARSING of the message template file. #
#########################################

# Extract lines which are not empty or comments
lines = File.read("./data/message_template.msg").lines
    .map{|l| l.gsub(%r{//(.*)$}, "")}
    .map(&:strip)
    .select{|l| not l.empty? and not l[0..1] == "//"}
version = lines.shift
puts "Generating structs for #{version}."

# Conversion from protocol spec types to Rust types.
DATA_TYPES = {
    "U8" => "u8", "U16" => "u16", "U32" => "u32", "U64" => "u64",
    "S8" => "i8", "S16" => "i16", "S32" => "i32", "S64" => "i64",
    "F32" => "f32", "F64" => "f64",
    "LLUUID" => "Uuid",
    "IPADDR" => "Ip4Addr", "IPPORT" => "Ip4Port",
    "LLVector3" => "Vector3<f32>", "LLVector3d" => "Vector3<f64>", "LLVector4" => "Vector4<f32>",
    "LLQuaternion" => "Quaternion<f32>",
    "BOOL" => "bool",
    "Variable" => "Vec<u8>"
}

class Field
    attr_accessor :count
    def initialize(name, type, count)
        @name = name
        @type = type
        @count = count
    end

    # rust version of name
    def r_name
        name = @name.underscore
        keywords = ["type", "override", "final"]
        if keywords.include? name
            name + "_"
        else
            name
        end
    end

    def ll_name
        @name
    end

    def r_type
        if DATA_TYPES.has_key? @type
            return DATA_TYPES[@type]
        elsif @type == "Fixed"
            return "[u8; #{@count}]"
        else
            raise "Unknown LL Type: #{@type.inspect}"
        end
    end

    def ll_type
        @type
    end

    def to_s
        "[Field: #{{name: @name, type: @type, count: @count}.inspect}]"
    end
end

class Block
    attr_accessor :fields, :quantity, :quantity_count

    def initialize(name, quantity, quantity_count, fields, message)
        @name = name
        @quantity = quantity
        @quantity_count = quantity_count
        @fields = fields
        @message = message
    end

    def ll_name
        @name
    end

    # Rust struct name.
    def r_name
        "#{@message.name}_#{@name}"
    end

    # Field name.
    def f_name
        @name.underscore
    end
end

# Extractor for blocks.
def extract_block(lines, message)
    blk_name, blk_quantity, blk_quantity_count = lines.shift.split
    blk_fields = []
    lines.each do |line|
        _, field_name, field_type, field_count, _ = line.split
        blk_fields << Field.new(field_name, field_type, field_count.to_i)
    end
    Block.new(blk_name, blk_quantity, blk_quantity_count, blk_fields, message)
end

# Extractor for messages.
# Gets an array of lines that have to be extracted.
def extract_message(lines)
    # Parse descriptor of message.
    msg_name, msg_freq, msg_id, msg_trust, msg_enc = lines.shift.split(" ")
    message = OpenStruct.new({
        name: msg_name,
        frequency: msg_freq,
        id: msg_id, # string, fixed id packages have an id of format 0xFFFF_FFFB which is parsed as string
        trust: msg_trust,
        encoding: msg_enc,
        blocks: []
    })

    # Remove opening and closing braces of block specifications.
    lines.shift
    lines.pop

    # Extract block specifications.
    block_raw = nil
    lines.each do |line|
        if block_raw.nil?
            block_raw = [line]
        elsif line == "{" or line == "}"
        elsif line[0] == "{"
            block_raw << line
        else
            message.blocks << extract_block(block_raw, message)
            block_raw = [line]
        end
    end
    unless block_raw.nil?
        message.blocks << extract_block(block_raw, message)
    end

    message
end

# Handle messages independently.
message_raw = nil
message_indentation = 0
messages = []
lines.each_with_index do |line, line_index|
    if line == "{" and message_indentation == 0
        message_raw = []
        message_indentation += 1
    elsif line == "{"
        message_indentation += 1
        message_raw << line
    elsif line == "}"
        message_indentation -= 1
        if message_indentation < 0
            puts "Too many closing braces at line no #{line_index}!"
            exit
        elsif message_indentation == 0
            messages << extract_message(message_raw)
        else
            message_raw << line
        end
    else
       message_raw << line
    end
end

###################
# CODE GENERATION #
###################
puts "Found specifications for #{messages.size} messages.\n\n"

def generate_struct(message)
    # Generate block definitions,
    # to avoid name clashes block names are {messagename}_{blockname}
    code = ""
    message.blocks.each do |block|
        code << "pub struct #{block.r_name} {\n"
        block.fields.each do |field|
            code << "\tpub #{field.r_name}: #{field.r_type},\n"
        end
        code << "}\n\n"
    end

    # Generate message definition.
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


# generate parser module.
# TODO

#####################
# Generate writers. #
#####################

# Returns the byte array for a given message that can be written after the header
# to indicate the type of the message.
def generate_message_id_bytes(message)
    full = message.id.to_i.to_s(16).rjust(8, "0")
    if message.frequency == "High"
        "[0x#{full[6..7]}]"
    elsif message.frequency == "Medium"
        "[0xff, 0x#{full[6..7]}]"
    elsif message.frequency == "Low"
        "[0xff, 0xff, 0x#{full[4..5]}, 0x#{full[6..7]}]"
    elsif message.frequency == "Fixed"
        full = message.id[2..9]
        "[0xff, 0xff, 0xff, 0x#{full[6..7]}]"
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
    if %w[u16 u32 u64 i16 i32 i64].include? r_type
        "try!(buffer.write_#{r_type}::<LittleEndian>(#{value}));\n"
    elsif %w[u8 i8].include? r_type
        "try!(buffer.write_#{r_type}(#{value}));\n"
    elsif %w[f32 f64].include? r_type
        "try!(buffer.write_#{r_type}::<LittleEndian>(#{value}));\n"
    elsif r_type == "Uuid"
        "try!(buffer.write(#{value}.as_bytes()));\n"
    elsif r_type == "Ip4Addr"
        "try!(buffer.write(&#{value}.octets()));\n"
    elsif r_type == "Ip4Port"
        "try!(buffer.write_u16::<LittleEndian>(#{value}));\n"
    elsif r_type == "Vector3<f32>" or r_type == "Vector3<f64>"
        f_type = r_type[8..10]
        "try!(buffer.write_#{f_type}::<LittleEndian>(#{value}.x));\n" +
        "try!(buffer.write_#{f_type}::<LittleEndian>(#{value}.y));\n" +
        "try!(buffer.write_#{f_type}::<LittleEndian>(#{value}.z));\n"
    elsif r_type == "Vector4<f32>"
        "try!(buffer.write_f32::<LittleEndian>(#{value}.x));\n" +
        "try!(buffer.write_f32::<LittleEndian>(#{value}.y));\n" +
        "try!(buffer.write_f32::<LittleEndian>(#{value}.z));\n" +
        "try!(buffer.write_f32::<LittleEndian>(#{value}.w));\n"
    elsif r_type == "Quaternion<f32>"
        puts "WARNING: Writing Quaternions is not yet implemented."
        nil
        # TODO: This one might be a bit more tricky:
        # can we just use a polar decomposition and discard the norm part?
        # I don't really know much about Quaternions so postponed until it
        # becomse a relevant issue.
    elsif r_type == "bool"
        "try!(buffer.write_u8(#{value} as u8));\n"
    elsif r_type == "Vec<u8>"
        "try!(buffer.write(&#{value}[..]));\n"
    elsif r_type[0...4] == "[u8;"
        "try!(buffer.write(&#{value}));\n"
    else
        raise "No rule for field writer generation of field: #{field}"
    end
end

def generate_message_impl(message)
    out = ""
    out << "impl Message for #{message.name} {\n"
    out << "\tfn write_to<W: Write>(&self, buffer: &mut W) -> WriteMessageResult {\n"
    out << "\t\t// Write the message number.\n"
    out << "\t\ttry!(buffer.write(&#{generate_message_id_bytes(message)}));\n"

    message.blocks.each do |block|
        if block.quantity == "Single"
            out << "\t\t// Block #{block.ll_name}\n"
            block.fields.each do |field|
                line = generate_field_writer(field, "self.#{block.f_name}")
                if line
                    out << "\t\t" + line
                else
                    puts "Didn't implement message: #{message.name}"
                    return ""
                end
            end
        else
            puts "Write implementation for blocks with quantity other than single not yet available."
            return ""
        end
    end

    out << "\tOk(())\n"
    out << "\t}\n"
    out << "}\n\n"
    out
end

# generate messages module.
File.open(TARGET_FILE, "w") do |file|
    file.write File.read("./data/preamble.rs")
    messages.each { |msg| file.write generate_struct(msg) }
    file.write "// Message IMPLEMENTATIONS\n\n\n\n"
    messages.each do |msg|
        code = generate_message_impl(msg)
        file.write code unless code.empty?
    end
end

if system 'which rustfmt'
    system 'rustfmt --write-mode overwrite ./src/messages.rs'
else
    puts "Warning: rustfmt not installed, please install and rerun!"
end

