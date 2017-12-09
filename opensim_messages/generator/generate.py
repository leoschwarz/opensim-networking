#!/usr/bin/env python3
from glob import glob
import itertools
import os
import sys
import textwrap

import gen.code
import gen.types
from gen.types import Message
sys.path.append("protocol/messages-util")
import message_xml

SCRIPT_DIR = os.path.dirname(__file__)
TARGET_FILE = os.path.join(SCRIPT_DIR, "../src/all.rs")
PREAMBLE_FILE = os.path.join(SCRIPT_DIR, "./preamble.rs")

def list_all_messages():
    """ Returns a list of all available messages. """
    def file_to_msgname(path):
        return os.path.splitext(os.path.basename(path))[0]

    msgs_1 = map(file_to_msgname, glob("protocol/messages/*.xml"))
    msgs_2 = map(file_to_msgname, glob("protocol/messages-original/*.xml"))
    return list(sorted(set(itertools.chain(msgs_1, msgs_2))))

def open_message_xml(msgname):
    """ Returns a file handle to the relevant file for the message with the specified name. """
    path_1 = os.path.join("protocol", "messages", "{}.xml".format(msgname))
    path_2 = os.path.join("protocol", "messages-original", "{}.xml".format(msgname))
    if os.path.exists(path_1):
        return open(path_1, "r")
    return open(path_2, "r")

def extract_message_xml(msgname):
    return message_xml.parse(open_message_xml(msgname), silence=True)

if __name__ == "__main__":
    all_msgnames = list_all_messages()
    print("Total number of messages: %s" % len(all_msgnames))

    # Extract message information.
    messages = [Message(extract_message_xml(msgname)) for msgname in all_msgnames]

    with open(TARGET_FILE, "w") as f:
        # Setup the preamble.
        f.write(textwrap.dedent("""\
                #![allow(non_snake_case)]
                #![allow(non_camel_case_types)]

                ///
                /// THIS FILE WAS AUTOMATICALLY GENERATED.
                /// Don't edit manually, instead edit the generator.
                ///\n\n"""))
        with open(PREAMBLE_FILE, "r") as preamble:
            f.write(preamble.read())

        for message in messages:
            f.write(gen.code.generate_struct(message))

        f.write(gen.code.generate_message_type_enum(all_msgnames))
        f.write(gen.code.generate_message_instance_enum(all_msgnames, messages))

        f.write("\n\n\n\n// BLOCK IMPLEMENTATIONS\n\n")
        for message in messages:
            for block in message.blocks:
                f.write(gen.code.generate_block_reader_impl(block))

        f.write("\n\n\n\n// MESSAGE IMPLEMENTATIONS\n\n")
        for message in messages:
            code = gen.code.generate_message_impl(message)
            f.write(code)

    # Format the file.
    os.system("rustup run nightly cargo fmt")

