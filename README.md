# IntentFuzzer - libAFL version

This is an automated greybox fuzzer for Intent receivers on Android.

## How To Use

`cargo run -- --help`

## Architecture 

```
  Fuzzer                Android Device/Emulator
  ┌───────────┐             ┌──────────────────┐
  │           │ TCP Port    │ App              │
  │ Collects  │ over ADB    │ ┌──────────────┐ │
  │ coverage ◄├─────────────┼►┤Coverage Agent│ │
  │           │             │ ├──────────────┤ │
  │           │             │ │              │ │
  │           │             │ │              │ │
  │           │             │ │              │ │
  │           │             │ │              │ │
  │           │             │ └──────▲───────┘ │
  │           │             │        │         │
  │ Mutates   │             │        │(Intents)│
  │ intents   │Sends Intents├────────┴─────────┤
  └───────────┴────────────►│ Android Activity │
                            │ Manager          │
                            └──────────────────┘
```

## Project Structure

[AndroidCoverageAgent](https://github.com/sslab-gatech/AndroidCoverageAgent) is
used to instrument apps on-device or on-emulator for coverage feedback.

The `apk_analyzer` subfolder contains a Kotlin project that uses the
[jadx](https://github.com/skylot/jadx) API to analyze an apk file and create
an `intent_template.json` file from it.

The root folder `.` contains the fuzzer written in Rust using
[libafl](https://github.com/AFLplusplus/LibAFL) to implement the fuzzing loop
and uses the generated `intent_template.json` and `adb` to communicate with the
coverage agent in the Android environment.
