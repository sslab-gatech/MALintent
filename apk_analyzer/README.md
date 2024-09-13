# Apk Analyzer

This is a Kotlin project that takes an Android `.apk` file, analyzes the
`AndroidManifest.xml` and performs some basic static analysis to generate an
`intent_template.json` file to be used with the IntentFuzzer.

## Usage

```bash
gradle run --args="/path/to/application.apk /path/to/output/directory/"
```
