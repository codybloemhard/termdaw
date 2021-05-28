-- Our lua file, where we do all the work
set_length(3.0);
set_render_samplerate(44100);
set_render_bitdepth(16);
set_output_file("outp.wav");

load_sample("snare", "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav");

load_midi("bassd", "/home/cody/git/music-gen/bassd.midi");
load_midi("snare", "/home/cody/git/music-gen/snare.midi");
 -- load_lv2("compressor", "http://calf.sourceforge.net/plugins/Compressor");

add_samplefloww("one", 1.0, 0.0, "snare", "snare", -1);
add_samplefloww("two", 1.0, 0.0, "kick", "bassd", -1);
add_normalize("sum", 1.0, 0.0);
-- add_lv2fx("effect", 1.0, 0.0, "compressor");

connect("one", "sum");
-- connect("crush", "sum");
connect("two", "sum");

set_output("sum");
