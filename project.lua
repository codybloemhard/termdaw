-- Our lua file, where we do all the work
set_length(3.0);
set_render_samplerate(44100);
set_render_bitdepth(16);
set_output_file("outp.wav");

load_sample("snare", "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav");

load_lv2("crusher", "http://calf.sourceforge.net/plugins/Crusher");

add_sampleloop("one", 1.0, 0.0, "snare");
add_sampleloop("two", 1.0, 0.0, "kick");
add_normalize("sum", 1.0, 0.0);
add_lv2fx("crush", 1.0, 0.0, "crusher");

connect("one", "crush");
connect("crush", "sum");
connect("two", "sum");

set_output("sum");
