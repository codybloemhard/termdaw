set_length(40.0);
set_render_samplerate(48000);
set_render_bitdepth(16);
set_output_file("outp.wav");

load_sample("snare", "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav");

load_midi_floww("bassd", "/home/cody/git/music-gen/bassd.midi");
load_midi_floww("snare", "/home/cody/git/music-gen/snare.midi");
load_midi_floww("comping", "/home/cody/git/music-gen/comping0.midi");
load_lv2("reverb", "http://calf.sourceforge.net/plugins/Reverb");
load_lv2("chorus", "http://calf.sourceforge.net/plugins/MultiChorus");
load_lv2("compressor", "http://calf.sourceforge.net/plugins/Compressor");
load_lv2("tape", "http://calf.sourceforge.net/plugins/TapeSimulator");

add_sample_lerp("kick", 1.0, 0.0, "kick", "bassd", -1, 40);
add_sample_lerp("snare", 1.0, 0.0, "snare", "snare", -1, 40);

hit_adsr = { 0.001, 0.02, 0.0, 0.0, 0.0, 0.0 }
note_adsr = { 0.01, 0.1, 0.8, 5.0, 0.2, 0.5 };
add_synth("comp", 0.3, 0.0, "comping", 0.5, 0.2, hit_adsr, 1.0, 0.7, note_adsr, 0.0, {});
add_adsr("env", 1.0, 0.0, 1.0, "snare", false, -1, { 0.01, 0.1, 0.8, 0.1, 0.2, 0.01 });

add_lv2fx("reverb", 1.0, 0.0, 0.9, "reverb");
add_lv2fx("chorus", 1.0, 0.0, 1.0, "chorus");
add_lv2fx("compress", 1.0, 0.0, 1.0, "compressor");
add_lv2fx("tape", 1.0, 0.0, 1.0, "tape");

add_normalize("sum", 1.0, 0.0);

connect("kick", "sum");
connect("snare", "env");
connect("env", "sum");
connect("comp", "chorus");
connect("chorus", "reverb");
connect("reverb", "compress");
connect("compress", "tape");
connect("tape", "sum");

set_output("sum");
