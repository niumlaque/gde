# Git Diff Extractor

_"Huh? What's git? I don't get it, I don't get it. I don't want to use anything like that. Just send me the updated files, and I'll overwrite them on our end. Oh, and I also want to see the differences, so please send the diff with the original files."_  
— Customer's response to my request for the repository URL

## Usage
```sh
Usage: gde [OPTIONS] --from <FROM COMMIT> --to <TO COMMIT> [TARGET REPO DIR]

Arguments:
  [TARGET REPO DIR]

Options:
      --git <GIT EXECUTABLE>  Path to git executable
      --from <FROM COMMIT>
      --to <TO COMMIT>
  -o, --output <OUTPUT DIR>
  -h, --help                  Print help
```
```sh
$ git clone https://github.com/niumlaque/gde.git /tmp/piyopiyo
$ gde --from 6a0453c --to 86ab16a -o /tmp /tmp/piyopiyo
git version: git version 2.30.2
target directory: /tmp/piyopiyo
root directory: /tmp/piyopiyo
output directory: /tmp/gde-a9c24409-828b-4991-a717-ba6fc8a7d1be
Updated files:
        src/bin/gde.rs
        src/git/gitdiff.rs
        src/git/mod.rs
current commit: 10a6ec373e98afcf8bd8d172122fded630b4db17
Copiying `from` files...
Copied: /tmp/piyopiyo/src/bin/gde.rs -> /tmp/gde-a9c24409-828b-4991-a717-ba6fc8a7d1be/from/src/bin/gde.rs
Copied: /tmp/piyopiyo/src/git/gitdiff.rs -> /tmp/gde-a9c24409-828b-4991-a717-ba6fc8a7d1be/from/src/git/gitdiff.rs
Copied: /tmp/piyopiyo/src/git/mod.rs -> /tmp/gde-a9c24409-828b-4991-a717-ba6fc8a7d1be/from/src/git/mod.rs
Copiying `to` files...
Copied: /tmp/piyopiyo/src/bin/gde.rs -> /tmp/gde-a9c24409-828b-4991-a717-ba6fc8a7d1be/to/src/bin/gde.rs
Copied: /tmp/piyopiyo/src/git/gitdiff.rs -> /tmp/gde-a9c24409-828b-4991-a717-ba6fc8a7d1be/to/src/git/gitdiff.rs
Copied: /tmp/piyopiyo/src/git/mod.rs -> /tmp/gde-a9c24409-828b-4991-a717-ba6fc8a7d1be/to/src/git/mod.rs
done
```