# iot_manager (official MonsGeek IOT bridge)

Unmodified files from `resources/app/iot_manager/` in the official
`MonsGeek_V4_Driver_500.3.4_WIN20260928.exe` installer (NSIS → `$PLUGINSDIR/app-32.7z`).
`APPVersion.json` in that build lists `IotManagerVersion` 0.1.8. These are MonsGeek's
binaries, not covered by this project's license.

`build.rs` deflates every `.exe`/`.dll` here into the app. Settings › IOT bridge unpacks them to
`%APPDATA%\Monsgeek App\iot_manager` and starts `iot_manager_rs.exe` hidden with no arguments,
the same way the official app does. It listens on 127.0.0.1:6015 (`iot_manager.IotManager`) and
starts `common_hid_rs.exe` on 127.0.0.1:3838 (`hid.HidService`), which is what the MonsGeek web
driver connects to. The app itself never uses the bridge.

To update: extract a newer installer, replace these files, update `VERSION` in `src/iot.rs`
and the hashes below.

SHA-256:

```
8c58e5fba22acc751032dfe97ce633e4f8a4c96089749bf316d55283b36649c2  HidSharp.dll
a0f2728f1734c236a9d02d9e25a88bc4f8cb7bd1faff1770726beb7af06bf8dc  LibreHardwareMonitorLib.dll
763eb4a7d4653f8cd0bfa0fbb640d1e8505d60b5872789ec68f25a9bbe59f826  ManagedLibreHardwareMonitor.dll
5c1405e8ddad2a907a113e09fe994dd85295db33d948da9527a2ff28157f2b91  ManagedLibreHardwareMonitorWrapper.dll
e1e27af7b07eeedf5ce71a9255f0422816a6fc5849a483c6714e1b472044fa9d  Newtonsoft.Json.dll
619fa555b598c058babb8dc11a242f5be6aec761b586dc3694362bc17ccec97a  common_hid_rs.exe
3892f0b5bd6ad1ebdfc3f448c951cb322d7767f166916406bc30fa3e3d1f1a2d  iot_manager_rs.exe
5b2389b7a2a5cd6ab7b99ea05d4ce32eec98b81bf28b6fb7aafab4d22c655df2  keyboard_monitor_service.exe
d40cde8e34216d77f2d07379a74edc5db0cdf2a2601c6316cc3ddad288ac9706  rhythm_service.exe
0838804a8bb24670d3765081119d6e4e5a726175eaeb6d2126fdd6a240a35a4d  screen_capture_service.exe
278d55428d4d3a47e6d097836fd0ba3d144602f83bd370fe7a25378ed97151ce  system_info_hid_rs.exe
```
