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
    "LLUUID" => "LLUUID",
    "IPADDR" => "IpAddr", "IPPORT" => "IpPort",
    "LLVector3" => "Vector3<f32>", "LLVector3d" => "Vector3<f64>", "LLVector4" => "Vector4<f32>",
    "LLQuaternion" => "Quaternion<f32>",
    "BOOL" => "bool"
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
        elsif @type == "Variable"
            return "Vec<u8>"
        elsif @type == "Fixed"
            return "[u8; #{@count}]"
        else
            raise "Unknown LL Type: #{@type.inspect}"
        end
    end

    def ll_type
        @type
    end
end

# Extractor for blocks.
def extract_block(lines)
    blk_name, blk_quantity, blk_quantity_count = lines.shift.split
    blk_fields = []
    lines.each do |line|
        _, field_name, field_type, field_count, _ = line.split
        blk_fields << Field.new(field_name, field_type, field_count.to_i)
    end
    OpenStruct.new({
        name: blk_name,
        quantity: blk_quantity,
        quantity_count: blk_quantity_count,
        fields: blk_fields
    })
end

# Extractor for messages.
# Gets an array of lines that have to be extracted.
def extract_message(lines)
    # Parse descriptor of message.
    msg_name, msg_freq, msg_trust, msg_enc = lines.shift.split(" ")

    # Remove opening and closing braces of block specifications.
    lines.shift
    lines.pop

    # Extract block specifications.
    msg_blocks = []
    block_raw = nil
    lines.each do |line|
        if block_raw.nil?
            block_raw = [line]
        elsif line == "{" or line == "}"
        elsif line[0] == "{"
            block_raw << line
        else
            msg_blocks << extract_block(block_raw)
            block_raw = [line]
        end
    end
    unless block_raw.nil?
        msg_blocks << extract_block(block_raw)
    end

    OpenStruct.new({
        name: msg_name,
        frequency: msg_freq,
        trust: msg_trust,
        encoding: msg_enc,
        blocks: msg_blocks
    })
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
        name = "#{message.name}_#{block.name}"
        code << "pub struct #{name} {\n"
        block.fields.each do |field|
            code << "\t#{field.r_name}: #{field.r_type},\n"
        end
        code << "}\n\n"
    end

    # Generate message definition.
    code << "pub struct #{message.name} {\n"

    message.blocks.each do |block|
        block_name = "#{message.name}_#{block.name}"
        if block.quantity.downcase == "single"
            code << "\t#{block.name.underscore}: #{block_name},\n"
        elsif block.quantity.downcase == "multiple"
            code << "\t#{block.name.underscore}: [#{block_name}; #{block.quantity_count}],\n"
        elsif block.quantity.downcase == "variable"
            code << "\t#{block.name.underscore}: Vec<#{block_name}>,\n"
        end
    end

    code << "}\n\n\n"
    code
end

# generate messages module.
File.open(TARGET_FILE, "w") do |file|
    file.write File.read("./data/preamble.rs")
    messages.each { |msg| file.write generate_struct(msg) } 
end

# generate parser module.
# TODO

if system 'which rustfmt'
    system 'rustfmt --write-mode overwrite ./src/messages.rs'
else
    puts "Warning: rustfmt not installed, please install and rerun!"
end

