#!/bin/bash
# This script pulls the newest documentation, regenerates the sources,
# and git adds things if invoked with the "gitadd" argument.

# update submodule
cd protocol
git pull
cd ..

# generate code
source pyenv/bin/activate
python generate.py
deactivate

# finish
if [ "$1" == "gitadd" ]
then
    git add protocol
    git add ../src/all.rs
fi

