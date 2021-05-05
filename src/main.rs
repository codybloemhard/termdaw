use mlua::prelude::*;

fn main() -> LuaResult<()> {
    let lua = Lua::new();

    let map_table = lua.create_table()?;

    let greet = lua.create_function(|_, name: String| {
        println!("Hello, {}!", name);
        Ok(())
    });

    map_table.set(1, "one")?;
    map_table.set("two", 2)?;

    lua.globals().set("map_table", map_table)?;
    lua.globals().set("greet", greet.unwrap())?;

    lua.load("for k,v in pairs(map_table) do print(k,v) end").exec()?;
    lua.load("greet(\"haha yes\")").exec()?;

    println!("Hello, world!");
    Ok(())
}

