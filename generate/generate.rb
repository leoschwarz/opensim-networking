#!/usr/bin/env ruby
require 'active_support/inflector'

############################################################################
# This is a code generator to create Rust structs for all messages defined #
# by the second life protocol.                                             #
############################################################################

cur_dir = File.dirname(__FILE__)
TARGET_FILE = File.expand_path(File.join(cur_dir, "../src/messages.rs"))
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
    file.write File.read(PREAMBLE_FILE)
    messages.each { |msg| file.write generate_struct(msg) }
    file.write "// Message IMPLEMENTATIONS\n\n\n\n"
    messages.each do |msg|
        code = generate_message_impl(msg)
        file.write code unless code.empty?
    end
end

if system 'which rustfmt'
    system "rustfmt --write-mode overwrite '#{TARGET_FILE}'"
else
    puts "Warning: rustfmt not installed, please install and rerun!"
end

