real: 584 entries parsed+indexed in 81 ms
      parse: 81 ms
real: view claims             28 ms  (77898 bytes)
real: view actors             23 ms  (120391 bytes)
real: view domains            23 ms  (4099 bytes)
real: view verdicts           12 ms  (2417 bytes)
real: view contradictions      2 ms  (2729 bytes)
synth: generated 100k entries in 24180 ms
synth: 100k parsed+indexed in 3930 ms
synth: parse: 3444 ms
synth: view claims           4842 ms  (19000703 bytes)
synth: view actors           2536 ms  (18279337 bytes)
synth: view domains          1182 ms  (1816 bytes)
synth: view verdicts          864 ms  (907 bytes)
synth: view contradictions   3250 ms  (16633964 bytes)
synth: full pipeline (parse+index+5 views) 7359 ms
synth: throughput 13589 entries/sec

Scale notes: parse+index is linear (~11us/entry at 100k). View materialization
cost is dominated by JSON serialization of full graph output, not graph
traversal. For interactive use at 100k+, prefer `verdicts`/`domains` (sub-second)
over full `claims` graph dumps.
