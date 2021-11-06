set_length(40.0);
set_render_samplerate(48000);
set_render_bitdepth(16);
set_output_file("outp.wav");

load_sample("snare", "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav");
load_sample("hihat", "/home/cody/doc/samples/drumnbass/hi-hats/closed.wav");
load_sample("ride", "/home/cody/doc/samples/drumnbass/ride/standard.wav");

declare_stream("ride");
declare_stream("hihat");
declare_stream("kick");
declare_stream("snare");

add_sample_lerp("kick", 5.0, 0.0, "kick", "bassd", -1, 40);
add_sample_lerp("snare", 0.9, 0.0, "snare", "snare", -1, 40);
add_sample_lerp("hihat", 0.3, 50.0, "hihat", "hihat", -1, 40);
add_sample_lerp("ride", 0.3, -50.0, "ride", "ride", -1, 40);

add_normalize("sum", 1.0, 0.0);

connect("kick", "sum");
connect("snare", "sum");
connect("hihat", "rsum");
connect("ride", "sum");

set_output("sum");
