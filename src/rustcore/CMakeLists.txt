cargo_build(NAME rustcore)

add_dependencies(rustcore cc_binding ccommon_rs pelikan_sys pelikan)
