# Content Provider

This content provider needs to be installed on the device to allow the fuzzer
to create files in a content provider. This app doesn't have a UI and only
needs to be installed; the fuzzer will handle the rest.

## Installation

The following command will build and install the content provider:

```bash
gradle installDebug
```
