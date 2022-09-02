# Negotiators library

Library for writing custom plugable negotiators for Yagna Agents.

## Roadmap

- [x] Basic Components API
- [x] Support for loading negotiators from shared libraries
- [x] Support for importing negotiators from statically linked libraries (library as Agent's dependency)
- [ ] Scoring Proposals
    - [x] Asynchronous negotiation decisions to enable time and Score based negotiations.
    - [x] CompositeComponent can collect Proposals for some period of time and choose the best from them
        - [x] CompositeComponent should be declarative configured in yaml. No support for custom strategies here
        - [ ] Components can set time hints, how mush Negotiator should wait before accepting Proposals. (Dynamic waiting time.)
- [ ] Advanced components API
    - [x] Events notification
        - [x] Proposal rejection
        - [x] Invoice events (Accepted, Rejected, Paid)
        - [x] Control events - interaction ability with components, change/query params and behavior
        - [ ] Shutdown
    - [x] Allowing components to read/write files in specified directory (persistence). (This could be useful for storing reputation records)
    - [ ] Allow setting some configuration values from code. Merge config loaded from yaml and overriden values from code. (Some values in Provider are set in command line and  don't want to move them to config file.)
- ~[ ] Rewrite shared libraries API to use C abi. (This will make it possible to integrate shared libraries with other languages.)~
- [ ] Support for Negotiators in binaries with RPC communication.
    - [ ] Local negotiator spawned in new process
    - [ ] Connect to exisitng negotiator (remote)
- [ ] Use `ya-negotiators` as Yagna Provider Agent dependency.
- [ ] Use `ya-negotiators` in yarapi (https://github.com/golemfactory/yarapi)
- [x] Testing Framework for components without need to use Agents
    - [x] Multi Provider/Requestor negotiations
    - [x] Recording negotiators' responses for making assertions
    - [x] Implement timeout for test negotiations
    - [x] Break infinite loops (set max number of negotiation steps)
    - [ ] Use matcher to check, if Proposals should be sent to all nodes or only to subset of them
    - [ ] Predefined testset to test components
- [ ] Example negotiators
    - [x] Builtin negotiators copied from Yagna Provider
    - [x] Example shared library negotiators filtering Nodes by name
    - [ ] Simple local reputation
    - [ ] Geo-localization components (both Provider/Requestor). Choosing the nearest node
    - [ ] Requestor-Provider ping component
    - [ ] Simple remote Nodes orchestrator example (choose which Node will pick up Offer)
- [ ] Declarative components building blocks
    - [ ] Negotiators Chain component
        - [ ] Allow to nest Negotiators
        - [ ] Allow to use seperate file descriptor to build the chain - Providers will be able to use predefined components chains or chains created by someone else. This will work like declarative mechanism of implementing components
    - [ ] Statistical and analitical components - generate data to use for scoring Offers
    - [ ] Expression building blocks
        - [ ] Copy-score component - for example: take value named "golem.stats.benchmark" and copy as "scorer.input.benchmark". This component will make connecting incompatibil components possible
        - [ ] Expression evaluator - write custom expression based on values passed from previous negotiators to compute score
    
    
    
