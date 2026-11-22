# Third-party notices

RA Hash Checker includes third-party software. Its license notices are reproduced below, as those licenses require.

## rcheevos

- Project: <https://github.com/RetroAchievements/rcheevos>
- Version: v12.5.0. It is vendored as a git submodule in `crates/rcheevos-hash/vendor/rcheevos`, and its hashing code is compiled into the app.
- License: MIT

```
MIT License

Copyright (c) 2018 RetroAchievements.org

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Other dependencies

The Rust crates (Tauri, reqwest, serde, zip, keyring, and others) and the npm packages (`@tauri-apps/*`, Vite, TypeScript, and others) are fetched by Cargo and npm when you build. Each keeps its own license, which you can find in `Cargo.lock` / `package-lock.json` and in each package's repository.
