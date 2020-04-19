# reverse-checksum-renamer

Renames files to its original name based on checksums found in catalog files (SFV or PAR2).

## Usage

    Usage: reverse-checksum-renamer [-i <input>] [-o <output>] <SFV/PAR2-files>
    -i  input directory
    -o  output directory
    -p  show referenced files in par2 or sfv file
        (--show-catalog)
    -f  find PAR2/SFV files and rename them
        (--fix-catalog-files)
    -c  only complete sets
    -g  group into subdirectories
    -v  verbose
    -d  dry run
    --degree-of-parallelism <number>
        maximum concurrent threads to calculate checksumes of files (0 is number of cores)
    --checksum-file
        print checksums of a file

### Arguments

    -i  if omitted the current directory will be the working path
    -o  if given all renamed files and the correcponding catalog files will be moved to that directory
    -f  use to find catalog files in all of the input files (determined by header and content)
    -g  if there are more than one catalog file, all to that catalog corresponding files will be 
        moved into a single directory below the output directory
    -c  only move files if all files referenced in the catalog file are found
    -d  do not move or rename files
    -p  list all referenced files in the given catalog file

### Notes

The 'SFV/PAR2-files' argument can be any file or even a '*' wildcard. If the fiule is not a recognised
catalog file, it will be ignored.

## Wait what ... why would I need this?

Consider having a bunch of files, which once have been part of a split archive. Due to a tragic
event in the space-time they lost their original names. Fortunately there exists a catalog file 
which contains the original filenames and a corresponding MD5 or SHA1 hash, which allows us to 
reconstruct the original filenames by comparing the hashes of the misnamed files to the hashes
of the catalog file.

## Thank you

This software uses the following libraries:

* https://crates.io/crates/crc
* https://crates.io/crates/md5
* https://crates.io/crates/num_cpus

Thank you for your work!

## Rust

Version 1.42.0