-- Our lua file, where we do all the work
set_length(40.0);
set_render_samplerate(48000);
set_render_bitdepth(16);
set_output_file("outp.wav");

-- some background sample that loops
load_sample("background", "/home/cody/temp/bg.wav");

load_midi_floww("bassd", "/home/cody/git/music-gen/bassd.midi");

-- add_sample_lerp("snare", 1.0, 0.0, "snare", "snare", -1, 40);

add_sampleloop("background", 1.0, 0.0, "background");
adsr = { 1.0, 0.01, 0.0, 0.5, 0.0, 0.0, 0.0, 0.1, 1.0 }
add_adsr("env", 1.0, 0.0, "bassd", false, -1, adsr);
add_normalize("sum", 1.0, 0.0);

connect("background", "env");
connect("env", "sum");

set_output("sum");
