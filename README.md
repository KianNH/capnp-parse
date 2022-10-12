# capnp-parse

This is just a fun project that'll try and extract
structs, enums, interfaces and their contents from `.capnp`
schemas into a `.json` file. There's lots of work to do, and
that's since this only really has one target - tracking changes
in [workerd](https://github.com/cloudflare/workerd), the open-source
Cloudflare Workers runtime.