# LZ77

A zero dependency, pure rust implementation of the FastLZ LZ77 compression algorithm.

Currently this library only decompresses, but compression will follow if there is a demand for it. This library was specifically built for the [ni-file](https://github.com/monomadic/ni-file) library, where sampler instruments built for Kontakt use an implementation of the LZ77 algorithm with very specific sliding window behaviors. It should (in theory) work for any LZ77 compressed file however. If it does not please file an issue.
