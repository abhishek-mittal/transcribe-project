# Changelog

## [0.1.0](https://github.com/abhishek-mittal/transcribe-project/compare/transcribe-app-v0.0.1...transcribe-app-v0.1.0) (2026-06-29)


### Features

* **api:** bypass YouTube bot challenge via PO token plugin + player_client fallback + cookie/proxy env hooks ([a140b2e](https://github.com/abhishek-mittal/transcribe-project/commit/a140b2eba0dc815f7683762b0a497a6c838f3be6))
* **api:** support Instagram via separate cookies jar ([35ab653](https://github.com/abhishek-mittal/transcribe-project/commit/35ab6532ab588721a7488490393cb39e51a91e3b))
* **desktop:** catch up Tauri desktop migration + bundle ffmpeg in sidecar ([5c52e25](https://github.com/abhishek-mittal/transcribe-project/commit/5c52e25108257e9fe2881c3559ed4c13f319e146))
* initial project scaffold — SvelteKit + Python Vercel function ([9bfef6f](https://github.com/abhishek-mittal/transcribe-project/commit/9bfef6f3d891f6340b6d3c702e169ccf45279c99))
* **search:** expand YouTube /results URLs into VideoPicker entries ([b2e8ac8](https://github.com/abhishek-mittal/transcribe-project/commit/b2e8ac817b7e4175bb7bd209bc5b6a7d698492c3))
* **stream:** real-time SSE streaming transcription with live animations ([d402c31](https://github.com/abhishek-mittal/transcribe-project/commit/d402c31ff4b7959482fb436e91e572ba7f7ee351))
* **ui:** typewriter animation, friendly messages, clean tab display ([f163bfa](https://github.com/abhishek-mittal/transcribe-project/commit/f163bfaf75b902638540765f0e617994a2727cd6))


### Bug Fixes

* **api:** only apply YouTube anti-bot tweaks for YouTube URLs ([f1dad32](https://github.com/abhishek-mittal/transcribe-project/commit/f1dad32e2b7e54b16118b146eabae669900d9884))
* **deploy:** guard Node.js 20 in redeploy.sh ([4d120e6](https://github.com/abhishek-mittal/transcribe-project/commit/4d120e6951d2f57f58e678fd0499f499967b1127))
* **deploy:** install Node.js 20 from NodeSource (Ubuntu 22.04 ships Node 12) ([20553a5](https://github.com/abhishek-mittal/transcribe-project/commit/20553a516c3b908ad93bed08e7c0dca369aeaecb))
* **deploy:** mark /opt/transcribe as git safe.directory in redeploy ([13dd34b](https://github.com/abhishek-mittal/transcribe-project/commit/13dd34b34e2b9da31316bf3460a8c744e1227c4c))
* **deploy:** restart bgutil-pot service on redeploy ([91d8ad1](https://github.com/abhishek-mittal/transcribe-project/commit/91d8ad16f8f86c1de5a5ece500399b071c5f52dd))
* **picker:** keep activity strip informative and pin action bar in view ([1d8ccc7](https://github.com/abhishek-mittal/transcribe-project/commit/1d8ccc7dab99216f8da93d846452db959e6840cd))
* **queue:** keep event listener alive for whole job; stamp activity-log ts ([5ce98ae](https://github.com/abhishek-mittal/transcribe-project/commit/5ce98ae89f4b1543122ba6be41cfd50aeb4c65e8))
* **search:** use ytsearch&lt;N&gt;: syntax (no brackets) ([2c70487](https://github.com/abhishek-mittal/transcribe-project/commit/2c70487b341cc2dc02a03002e51654a03d5d24e0))
* **sidecar:** patch platform.mac_ver() to prevent yt-dlp plugin-discovery crash ([37b703d](https://github.com/abhishek-mittal/transcribe-project/commit/37b703d55fe408daaa87520d27ca5fbad46dc02b))
* **ui:** eliminate 3-way bind:activeTab that froze Queue view ([659eb79](https://github.com/abhishek-mittal/transcribe-project/commit/659eb792b035f2130f690271726287647ed1ad19))
* **yt-dlp:** add fetch_pot=always + player_skip=webpage to extractor_args ([e893eec](https://github.com/abhishek-mittal/transcribe-project/commit/e893eec464d31cd04612d4d4602255c350ebd5f6))


### Performance Improvements

* **search:** drop default count to 20, loosen socket timeout ([1d192a1](https://github.com/abhishek-mittal/transcribe-project/commit/1d192a1fc4097c91f7e7025fca542e1499b18b1a))
