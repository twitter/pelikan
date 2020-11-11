cargo_build(NAME rustcore)

add_dependencies(rustcore ccommon_rs ccommon_sys pelikan_sys pelikan)
