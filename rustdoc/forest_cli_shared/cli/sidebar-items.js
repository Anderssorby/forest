window.SIDEBAR_ITEMS = {"enum":[["ConfigPath",""],["SnapshotServer","Snapshot fetch service provider"]],"fn":[["chain_path","Gets chain data directory"],["check_for_unknown_keys",""],["cli_error_and_die","Print an error message and exit the program with an error code Used for handling high level errors such as invalid parameters"],["db_path","Gets database directory"],["default_snapshot_dir",""],["is_aria2_installed","Checks whether `aria2c` is available in PATH"],["is_car_or_tmp",""],["normalize_filecoin_snapshot_name","Returns a normalized snapshot name Filecoin snapshot files are named in the format of `<height>_<YYYY_MM_DD>T<HH_MM_SS>Z.car`. Normalized snapshot name are in the format `filecoin_snapshot_{mainnet|calibnet}_<YYYY-MM-DD>_height_<height>.car`."],["snapshot_fetch","Fetches snapshot from a trusted location and saves it to the given directory. Chain is inferred from configuration."],["to_size_string","convert `BigInt` to size string using byte size units (i.e. KiB, GiB, PiB, etc) Provided number cannot be negative, otherwise the function will panic."]],"static":[["FOREST_VERSION_STRING",""]],"struct":[["CliOpts","CLI options"],["Client",""],["Config",""],["DaemonConfig","Structure that defines daemon configuration when process is detached"],["FilecoinSnapshotFetchConfig",""],["ForestFetchConfig",""],["ForestSnapshotFetchConfig",""],["LogConfig",""],["LogValue",""],["SnapshotFetchConfig",""],["SnapshotInfo","Snapshot attributes"],["SnapshotStore","Collection of snapshots"]]};