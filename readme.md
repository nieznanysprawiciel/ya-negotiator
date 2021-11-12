# Negotiators library

Library for writing custom plugable negotiators for Yagna Agents.

## Roadmap

- [x] Basic Components API
- [x] Support for loading negotiators from shared libraries
- [x] Support for importing negotiators from statically linked libraries (library as Agent's dependency)
- [ ] Scoring Proposals
    - [x] Asynchronous negotiation decisions to enable time and Score based negotiations.
    - [ ] CompositeComponent can collect Proposals for some period of time and choose the best from them
        - [ ] CompositeComponent should be declarative configured in yaml. No support for custom strategies here
        - [ ] Components can set time hints, how mush Negotiator should wait before accepting Proposals. (Dynamic waiting time.)
- [ ] Advanced components API
    - [x] Events notification
        - [x] Proposal rejection
        - [x] Invoice events (Accepted, Rejected, Paid)
    - [x] Allowing components to read/write files in specified directory (persistence). (This could be useful for storing reputation records or example.)
    - [ ] Allow setting some configuration values from code. Merge config loaded from yaml and overriden values from code. (Some values in Provider are set in command line and  don't want to move them to config file.)
- [ ] Rewrite shared libraries API to use C abi. (This will make it possible to integrate shared libraries with other languages.)
- [ ] Support for Negotiators in binaries with RPC communication.
- [ ] Use `ya-negotiators` as Yagna Provider Agent dependency.
- [ ] Use `ya-negotiators` in yarapi (https://github.com/golemfactory/yarapi)
- [x] Testing Framework for components without need to use Agents
    - [x] Multi Provider/Requestor negotiations
    - [x] Recording negotiators' responses for making assertions
    - [x] Implement timeout for test negotiations
    - [x] Break infinite loops (set max number of negotiation steps)
    - [ ] Use matcher to check, if Proposals should be sent to all nodes or only to subset of them
- [ ] Example negotiators
    - [x] Builtin negotiators copied from Yagna Provider
    - [x] Example shared library negotiators filtering Nodes by name
    - [ ] Simple local reputation
    - [ ] Geo-localization components (both Provider/Requestor). Choosing the nearest node
    
    
    
