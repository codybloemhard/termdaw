-- Our lua file, where we do all the work
set_length(10.0);
set_render_samplerate(48000);
set_render_bitdepth(16);
set_output_file("outp.wav");

load_sample("snare", "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav");

load_midi_floww("bassd", "/home/cody/git/music-gen/bassd.midi");
load_midi_floww("snare", "/home/cody/git/music-gen/snare.midi");
load_midi_floww("test", "/home/cody/git/music-gen/comping0.midi");
-- load_lv2("compressor", "http://calf.sourceforge.net/plugins/Compressor");

add_samplefloww_lerp("kick", 1.0, 0.0, "kick", "bassd", -1, 40);
add_samplefloww_lerp("snare", 1.0, 0.0, "snare", "snare", -1, 40);
add_sinefloww("comp", 0.2, 0.0, "test");
add_adsr("env", 1.0, 0.0, "snare", false, -1, { 0.01, 0.1, 0.8, 0.1, 0.2, 0.01 });
add_normalize("sum", 1.0, 0.0);

connect("kick", "sum");
connect("snare", "env");
connect("env", "sum");
--connect("comp", "sum");

set_output("sum");
