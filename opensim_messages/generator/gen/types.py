####################################################################
# Type conversions and class definitions for the parsed XML files. #
####################################################################
from inflector import Inflector
import textwrap

# Conversion from the types as used in the XML specs to Rust types.
TYPE_CONVERSIONS = {
    "U8": "u8", "U16": "u16", "U32": "u32", "U64": "u64",
    "S8": "i8", "S16": "i16", "S32": "i32", "S64": "i64",
    "F32": "f32", "F64": "f64",
    "LLUUID": "Uuid",
    "IPADDR": "Ip4Addr", "IPPORT": "IpPort",
    "LLVector3": "Vector3<f32>", "LLVector3d": "Vector3<f64>", "LLVector4": "Vector4<f32>",
    "LLQuaternion": "Quaternion<f32>",
    "BOOL": "bool",
    "Variable 1": "Vec<u8>", "Variable 2": "Vec<u8>",
}

def to_rust_doc(raw_doc):
    """ Convert XML raw doc element to Rust doc. """
    # TODO handle docs with multiple children (e.g. with refs etc)
    dedented = textwrap.dedent(raw_doc.content_[0].getValue())
    return "\n".join(["/// " + line for line in dedented.splitlines()])

class Field:
    def __init__(self, xml_obj):
        self._name = xml_obj.name
        self._type = xml_obj.type_

        if xml_obj.type_.startswith("Fixed"):
            self._type, self.count = xml_obj.type_.split()
        else:
            self._type = xml_obj.type_

    @property
    def r_name(self):
        """ Rust version of the name. """
        name = Inflector().underscore(self._name)
        if name in ("type", "override", "final"):
            return name + "_"
        else:
            return name

    @property
    def ll_name(self):
        return self._name

    @property
    def r_type(self):
        if self._type in TYPE_CONVERSIONS:
            return TYPE_CONVERSIONS[self._type]
        elif self._type == "Fixed":
            return "[u8; {}]".format(self.count)
        else:
            raise RuntimeError("Unknown LL Type: {}".format(self._type))

    @property
    def ll_type(self):
        return self._type

class Block:
    def __init__(self, xml_obj, message):
        self._name = xml_obj.name
        self.fields = [Field(f) for f in xml_obj.field]
        self._message = message

        parts = xml_obj.quantity.split()
        self.quantity = parts[0]
        if len(parts) > 1:
            self.quantity_count = int(parts[1])

    @property
    def ll_name(self):
        return self._name

    @property
    def r_name(self):
        """
        The name of the Rust struct, the message name is included to avoid name clashes of
        blocks with the same name but different specifications.
        """
        return self._message.name + "_" + self._name

    @property
    def f_name(self):
        return Inflector().underscore(self._name)

class Message:
    def __init__(self, xml_obj):
        # LL and Rust version share the same name.
        self.name = xml_obj.name
        self.frequency_class = xml_obj.frequency_class # TODO: combine into the previous frequency field?
        self.frequency_number = xml_obj.frequency_number
        # TODO ? self.id = 
        self.trusted = xml_obj.trusted
        self.compression = xml_obj.compression
        self.blocks = [Block(block, self) for block in xml_obj.block]
        self.doc = to_rust_doc(xml_obj.doc)

    @property
    def message_num(self):
        bs = [self.id_byte(n)[2:4] for n in reversed(range(4))]
        return "0x" + "".join(bs)

    def id_byte(self, n):
        """
        Returns a hexadecimal string (with '0x' prefix) of the n-th byte of the id.
        Bytes are numbered from left to right in the spec, i.e. for High frequency messages
        there is only byte 0, for medium byte 0 is 0xff, while there is also byte 1 etc.
        """
        if not hasattr(self, "_full_id"):
            if self.frequency_number.startswith("0x"):
                self._full_id = self.frequency_number.lower()
            else:
                self._full_id = "{0:08x}".format(int(self.frequency_number))
        full = self._full_id
        if self.frequency_class == "high":
            if n == 0:
                return "0x" + full[6:8]
            else:
                return "0x00"
        elif self.frequency_class == "medium":
            if n == 0:
                return "0xff"
            elif n == 1:
                return "0x" + full[6:8]
            else:
                return "0x00"
        elif self.frequency_class == "low":
            if n == 0 or n == 1:
                return "0xff"
            elif n == 2:
                return "0x" + full[4:6]
            elif n == 3:
                return "0x" + full[6:8]
            else:
                return "0x00"
        elif self.frequency_class == "fixed":
            full = self.frequency_number[2:10]
            if 0 <= n and n <= 2:
                return "0xff"
            elif n == 3:
                return "0x" + full[6:8]
            else:
                return "0x00"
        else:
            raise RuntimeError("Invalid message frequency for msg: %s" % msg.__dict__)
