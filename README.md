# ExposureBracketingOrganizer

![Demo GIF](static/demo.gif)

ExposureBracketingOrganizer is a GUI application designed to streamline the process of organizing bracketed exposures. It automatically detects sequences of images taken with varying exposure values (EVs) and moves them into nested folder. This organization makes it significantly easier to process these bracketed sets with other software.

## Usage

You have to recreate the Exposure bracketing settings of your camera. If you don't know it, you can just discover them using the "Get Exposure Bias" Button.

## Under the Hood

ExposureBracketingOrganizer leverages the excellent `rawler` library by `dnglab` ([https://crates.io/crates/rawler](https://crates.io/crates/rawler)) for robust RAW file parsing capabilities.