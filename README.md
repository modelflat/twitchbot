# my dank twitch chat bot 

[![Build Status](https://travis-ci.com/modelflat/twitchbot.svg?branch=master)](https://travis-ci.com/modelflat/twitchbot)

### TODO:

* Split codebase into library and binary parts
* Refactor `core.rs` so the TMI is properly abstracted
* Get rid of `pest` (?)
* Command system (with permissions)
* Receive channel timeouts from ROOMSTATE/USERSTATE
* Proper logging
* Figure out why sometimes messages are sent too quickly (might be bug in EventQueue) 
