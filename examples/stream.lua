load_sample("snare", "/home/cody/doc/samples/drumnbass/snare-2-damped/snare-2-dampened-v-2.wav", "mix-down");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav", "mix-down");
load_sample("hihat", "/home/cody/doc/samples/drumnbass/hi-hats/tight.wav", "normalize-seperate");
load_sample("ride", "/home/cody/doc/samples/drumnbass/ride/standard.wav", "normalize-seperate");

declare_stream("ride");
declare_stream("hihat");
declare_stream("kick");
declare_stream("snare");
declare_stream("chords");

add_sample_lerp("kick", 1.0, 0.0, "kick", "kick", -1, 40);
add_sample_lerp("snare", 1.0, 0.0, "snare", "snare", -1, 40);
add_sample_lerp("hihat", 3.0, 20.0, "hihat", "hihat", -1, 40);
add_sample_lerp("ride", 1.0, -20.0, "ride", "ride", -1, 40);

hit_adsr = { 0.001, 0.02, 0.0, 0.0, 0.0, 0.0 };
note_adsr = { 0.01, 0.1, 0.8, 5.0, 0.2, 0.5 };
add_synth("chords", 0.5, 0.0, "chords", 0.4, 0.3, hit_adsr, 1.0, 0.8, note_adsr, 0.0, {});

add_normalize("sum", 1.0, 0.0);

connect("kick", "sum");
connect("snare", "sum");
connect("hihat", "sum");
connect("ride", "sum");
connect("chords", "sum");

set_output("sum");
