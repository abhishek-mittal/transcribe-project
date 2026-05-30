# Changelog

## [0.1.0](https://github.com/abhishek-mittal/transcribe-project/compare/transcribe-app-v0.0.1...transcribe-app-v0.1.0) (2026-05-30)


### Features

* **api:** bypass YouTube bot challenge via PO token plugin + player_client fallback + cookie/proxy env hooks ([a140b2e](https://github.com/abhishek-mittal/transcribe-project/commit/a140b2eba0dc815f7683762b0a497a6c838f3be6))
* **api:** support Instagram via separate cookies jar ([35ab653](https://github.com/abhishek-mittal/transcribe-project/commit/35ab6532ab588721a7488490393cb39e51a91e3b))
* initial project scaffold — SvelteKit + Python Vercel function ([9bfef6f](https://github.com/abhishek-mittal/transcribe-project/commit/9bfef6f3d891f6340b6d3c702e169ccf45279c99))


### Bug Fixes

* **api:** only apply YouTube anti-bot tweaks for YouTube URLs ([f1dad32](https://github.com/abhishek-mittal/transcribe-project/commit/f1dad32e2b7e54b16118b146eabae669900d9884))
* **deploy:** guard Node.js 20 in redeploy.sh ([4d120e6](https://github.com/abhishek-mittal/transcribe-project/commit/4d120e6951d2f57f58e678fd0499f499967b1127))
* **deploy:** install Node.js 20 from NodeSource (Ubuntu 22.04 ships Node 12) ([20553a5](https://github.com/abhishek-mittal/transcribe-project/commit/20553a516c3b908ad93bed08e7c0dca369aeaecb))
* **deploy:** mark /opt/transcribe as git safe.directory in redeploy ([13dd34b](https://github.com/abhishek-mittal/transcribe-project/commit/13dd34b34e2b9da31316bf3460a8c744e1227c4c))
* **deploy:** restart bgutil-pot service on redeploy ([91d8ad1](https://github.com/abhishek-mittal/transcribe-project/commit/91d8ad16f8f86c1de5a5ece500399b071c5f52dd))
* **yt-dlp:** add fetch_pot=always + player_skip=webpage to extractor_args ([e893eec](https://github.com/abhishek-mittal/transcribe-project/commit/e893eec464d31cd04612d4d4602255c350ebd5f6))
