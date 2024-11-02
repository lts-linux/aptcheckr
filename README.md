# Aptcheckr

_Aptcheckr_ is a command line tool to verify apt repositories.
It's based in [libapt](https://lts-linux.eu/projects/libapt/)
and verifies that all apt repository metadata is compliant with
the [Debian policy](https://www.debian.org/doc/debian-policy/),
and that the package set is consistent, i.e. all dependencies
and source packages are available.
