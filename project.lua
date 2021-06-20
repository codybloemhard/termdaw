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

add_samplefloww_multi("one", 1.0, 0.0, "snare", "snare", -1);
add_samplefloww_multi("two", 1.0, 0.0, "kick", "bassd", -1);
add_sinefloww("three", 1.0, 0.0, "test");
-- add_sampleloop("one", 1.0, 0.0, "snare");
-- add_sampleloop("two", 1.0, 0.0, "kick");
add_normalize("sum", 1.0, 0.0);
-- add_lv2fx("effect", 1.0, 0.0, "compressor");

connect("one", "sum");
-- connect("crush", "sum");
connect("two", "sum");
connect("three", "sum");

set_output("sum");
