This directory contains the script to generate structs and trait implementations for all messages
specified in the `message_template.msg` file.

## Requirements
* `ruby 2.3` (other versions of the ruby 2.\* branch will probably work as well)
* `activesupport` (install with `gem install activesupport`)
* `rustfmt` (install with `cargo install rustfmt`)

## Running
Just execute the file `./generator/generate.rb` and it should handle everything.  
Rerun the generator script after making changes to it and check if it created the anticipated changes or
preserved the current messages file if nothing was supposed to change.

