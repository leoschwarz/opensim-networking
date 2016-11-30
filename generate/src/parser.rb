#########################################
# PARSING of the message template file. #
#########################################

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
    message = Message.new({
        name: msg_name,
        frequency: msg_freq,
        id: msg_id,
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

def parse_messages(file_path)
    # Extract lines which are not empty or comments
    lines = File.read(file_path).lines
        .map{|l| l.gsub(%r{//(.*)$}, "")}
        .map(&:strip)
        .select{|l| not l.empty? and not l[0..1] == "//"}
    version = lines.shift
    puts "Generating structs for #{version}."

    # Handle the messages
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
    messages
end

