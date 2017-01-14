########################################################################
# Type conversions and class definitions for the parsed template file. #
########################################################################

# Conversion from protocol spec types to Rust types.
TYPE_CONVERSIONS = {
    "U8" => "u8", "U16" => "u16", "U32" => "u32", "U64" => "u64",
    "S8" => "i8", "S16" => "i16", "S32" => "i32", "S64" => "i64",
    "F32" => "f32", "F64" => "f64",
    "LLUUID" => "Uuid",
    "IPADDR" => "Ip4Addr", "IPPORT" => "IpPort",
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
        if TYPE_CONVERSIONS.has_key? @type
            return TYPE_CONVERSIONS[@type]
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
    # We are combing the message name with the block name to avoid name clashes of blocks
    # with the same name but different specifications.
    def r_name
        "#{@message.name}_#{@name}"
    end

    # Field name.
    def f_name
        @name.underscore
    end
end

class Message
    attr_accessor :blocks, :id, :frequency
    # Both LL and Rust version are exactly the same.
    attr_accessor :name

    def initialize(fields)
        @name = fields[:name]
        @frequency = fields[:frequency]
        # note this is a string as fixed id messages have id of format 0xFFFF_FFFX.
        @id = fields[:id]
        @trust = fields[:trust]
        @encoding = fields[:encoding]
        @blocks = fields[:blocks]
    end
end

