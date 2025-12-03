# ExposureBracketingOrganizer

![Banner](static/banner.jpg)

ExposureBracketingOrganizer is a GUI application designed to streamline the process of organizing bracketed exposures. It automatically detects sequences of images taken with varying exposure values (EVs) and moves them into nested folder. This organization makes it significantly easier to process these bracketed sets with other software like [HDRMerge](https://jcelaya.github.io/hdrmerge/).

## Usage
![Screenshot](static/screenshot.png)

You have to recreate the Exposure bracketing settings of your camera. If you don't know it, you can just discover them using the "Get Exposure Bias" Button.

## File Ordering

The application processes files using "[Natural String Ordering](https://crates.io/crates/natord)" (e.g., `A6401473.ARW`, `A6401474.ARW`, ..., `A6401480.ARW`). This ensures that sequences are detected correctly.

Default camera filenames usually follow this ordering naming schema. However, **when a custom file name is used**, the ordering might not work as expected, leading to issues in sequence detection.


## Under the Hood

ExposureBracketingOrganizer leverages the excellent [`rawler`](https://crates.io/crates/rawler) library by `dnglab` for robust RAW file parsing capabilities.