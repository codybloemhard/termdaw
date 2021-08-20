-- Our lua file, where we do all the work
set_length(40.0);
set_render_samplerate(48000);
set_render_bitdepth(16);
set_output_file("outp.wav");

-- some background sample that loops
load_sample("background", "/home/cody/temp/bg.wav");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav");

load_midi_floww("bassd", "/home/cody/git/music-gen/bassd.midi");

add_sample_lerp("kick", 1.0, 0.0, "kick", "bassd", -1, 40);
add_sampleloop("background", 1.0, 0.0, "background");
dip = 0.3;
adsr = { 1.0, 0.01, dip, 0.2, dip, 0.0, 0.0, 0.05, 1.0 };
add_adsr("env", 1.0, 0.0, 1.0, "bassd", false, false, -1, adsr);
add_normalize("sum", 1.0, 0.0);

connect("kick", "sum");
connect("background", "env");
connect("env", "sum");

set_output("sum");
