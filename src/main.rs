use mlua::prelude::*;
use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };

mod sample;
mod graph;
use sample::*;
use graph::*;

fn main() -> Result<(), String>{
    let setup_lua = "set_length(3.0)\nset_render_samplerate(44100)";
    let lua = Lua::new();

    let (bl, psr, cs, render_sr, bd) = {
        let mut bl = 1024;
        let mut psr = 48000;
        let mut frames = 0;
        let mut render_sr = 48000;
        let mut bd = 16;
        lua.scope(|scope| {
            lua.globals().set("set_bufferlen", scope.create_function_mut(|_, new_bl: usize| { bl = new_bl; Ok(()) })?,)?;
            lua.globals().set("set_samplerate", scope.create_function_mut(|_, new_sr: usize| { psr = new_sr; Ok(()) })?,)?;
            lua.globals().set("set_length", scope.create_function_mut(|_, new_frames: usize| { frames = new_frames; Ok(()) })?,)?;
            lua.globals().set("set_render_samplerate", scope.create_function_mut(|_, new_sr: usize| { render_sr = new_sr; Ok(()) })?,)?;
            lua.globals().set("set_render_bitdepth", scope.create_function_mut(|_, new_bd: usize| { bd = new_bd; Ok(()) })?,)?;
            lua.load(setup_lua).exec()
        }).unwrap();
        (bl, psr, ((psr * frames) as f32 / bl as f32).ceil() as usize, render_sr, bd)
    };

    println!("{}", render_sr);

    let mut sb = SampleBank::new(psr);
    sb.add("snare".to_owned(), "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav")?;
    sb.add("kick".to_owned(), "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav")?;
    let mut g = Graph::new(bl);

    g.add(Vertex::new(bl, 1.0, 0.0, VertexExt::sample_loop(sb.get("snare").unwrap())), "one".to_owned());
    g.add(Vertex::new(bl, 1.0, 0.0, VertexExt::sample_loop(sb.get("kick").unwrap())), "two".to_owned());
    g.add(Vertex::new(bl, 1.0, 0.0, VertexExt::normalize(true)), "sum".to_owned());
    g.connect("one", "sum");
    g.connect("two", "sum");
    g.set_output("sum");
    if !g.check_graph(){
        return Err("TermDaw: no output vertex found.".to_owned());
    }
    g.scan(cs);

    let (msr, mbd) = sb.get_max_sr_bd();
    if psr > render_sr{
        println!("TermDaw: warning: render will down sample from {}(project s.r.) to {}.", psr, render_sr);
    }
    if msr > render_sr{
        println!("TermDaw: warning: render will down sample from peak input quality({}) to {}.", msr, render_sr);
    }
    if !(bd == 8 || bd == 16 || bd == 24 || bd == 32) { panic!("Bitdepth of {} not supported: choose bitdepth in {{8, 16, 24, 32}}.", bd); }
    if mbd > bd{
        println!("TermDaw: warning: render will lose bitdepth from peak input quality({} bits) to {} bits", mbd, bd);
    }
    let spec = hound::WavSpec{
        channels: 2,
        sample_rate: render_sr as u32,
        bits_per_sample: bd as u16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create("outp.wav", spec).unwrap();
    let amplitude = if bd < 32 { ((1 << (bd - 1)) - 1) as f32 }
    else { i32::MAX as f32 };
    fn write_16s<T: std::io::Write + std::io::Seek>(writer: &mut hound::WavWriter<T>, l: &[f32], r: &[f32], len: usize, amplitude: f32){
        for i in 0..len{
            writer.write_sample((l[i] * amplitude) as i16).unwrap();
            writer.write_sample((r[i] * amplitude) as i16).unwrap();
        }
    }
    fn write_32s<T: std::io::Write + std::io::Seek>(writer: &mut hound::WavWriter<T>, l: &[f32], r: &[f32], len: usize, amplitude: f32){
        for i in 0..len{
            writer.write_sample((l[i] * amplitude) as i32).unwrap();
            writer.write_sample((r[i] * amplitude) as i32).unwrap();
        }
    }
    if psr > render_sr{
        let params = InterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: InterpolationType::Nearest,
            oversampling_factor: 160,
            window: WindowFunction::BlackmanHarris2,
        };
        let mut resampler = SincFixedIn::<f32>::new(
            render_sr as f64 / psr as f64,
            params, bl, 2
        );
        for _ in 0..cs{
            let chunk = g.render();
            if chunk.is_none() { continue; }
            let chunk = chunk.unwrap();
            let waves_in = vec![chunk.l.clone(), chunk.r.clone()];
            let waves_out = resampler.process(&waves_in).unwrap();
            if bd > 16 { write_32s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
            else { write_16s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
        }
    } else {
        for _ in 0..cs{
            let chunk = g.render();
            if chunk.is_none() { continue; }
            let chunk = chunk.unwrap();
            if bd > 16 { write_32s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
            else { write_16s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
        }
    }

    Ok(())
}

// fn lua_test() -> LuaResult<()> {
//     let lua = Lua::new();
//
//     let map_table = lua.create_table()?;
//
//     let greet = lua.create_function(|_, name: String| {
//         println!("Hello, {}!", name);
//         Ok(())
//     });
//
//     map_table.set(1, "one")?;
//     map_table.set("two", 2)?;
//
//     lua.globals().set("map_table", map_table)?;
//     lua.globals().set("greet", greet.unwrap())?;
//
//     lua.load("for k,v in pairs(map_table) do print(k,v) end").exec()?;
//     lua.load("greet(\"haha yes\")").exec()?;
//
//     println!("Hello, world!");
//     Ok(())
// }

