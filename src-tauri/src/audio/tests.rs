#[cfg(test)]
mod tests {
    use crate::audio::processor::LooperProcessor;
    use crate::audio::track::LoopTrack;
    use crate::audio::types::{AudioCommand, LooperSnapshot, TrackState};
    use crossbeam_channel::unbounded;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_track_recording_and_looping() {
        let sample_rate = 44100;
        let mut track = LoopTrack::new(0, sample_rate);
        assert_eq!(track.state, TrackState::Empty);

        // Start recording
        track.start_recording();
        assert_eq!(track.state, TrackState::Recording);

        // Record 1000 stereo frames (simulate 1kHz sine wave)
        for i in 0..1000 {
            let sample = (i as f32 * 0.1).sin();
            track.record_frame(sample, sample);
        }

        assert_eq!(track.length_frames, 1000);
        assert_eq!(track.buffer.len(), 2000);

        // Finish recording -> Transitions to Playing
        track.finish_recording();
        assert_eq!(track.state, TrackState::Playing);
        assert_eq!(track.playhead, 0);

        // Read and advance 1000 frames
        for _ in 0..1000 {
            let (l, r, _) = track.read_and_advance(false);
            assert_eq!(l, r);
        }

        // Loop should have wrapped back to 0
        assert_eq!(track.playhead, 0);
    }

    #[test]
    fn test_track_seam_crossfade() {
        let mut track = LoopTrack::new(0, 44100);
        track.start_recording();

        // Feed step function: 1.0 at start, -1.0 at end to create an intentional step discontinuity
        for i in 0..1000 {
            if i < 500 {
                track.record_frame(1.0, 1.0);
            } else {
                track.record_frame(-1.0, -1.0);
            }
        }

        track.finish_recording();

        // The seam between end (-1.0) and start (1.0) should now be smoothed by micro-crossfade
        let start_l = track.buffer[0];
        let end_l = track.buffer[(track.length_frames - 1) * 2];

        // Should not be hard -1.0 and 1.0 at the very edges due to crossfade blending
        assert!(start_l < 1.0);
        assert!(end_l > -1.0);
    }

    #[test]
    fn test_overdub_and_undo() {
        let mut track = LoopTrack::new(0, 44100);
        track.start_recording();

        // Record base loop of 100 frames with 0.5
        for _ in 0..100 {
            track.record_frame(0.5, 0.5);
        }
        track.finish_recording();

        // Start overdub
        track.start_overdub();
        assert_eq!(track.state, TrackState::Overdubbing);
        assert!(track.undo_buffer.is_some());

        // Overdub extra 0.2 onto first 10 frames
        for _ in 0..10 {
            track.overdub_frame(0.2, 0.2);
            track.read_and_advance(false);
        }

        // Buffer should now have 0.5 + 0.2 = ~0.7
        assert!((track.buffer[0] - 0.7).abs() < 1e-4);

        // Perform undo
        track.undo();
        // Buffer should be reverted to original 0.5
        assert!((track.buffer[0] - 0.5).abs() < 1e-4);
        assert!(track.undo_buffer.is_none());
    }

    #[test]
    fn test_processor_multi_track_sync() {
        let sample_rate = 44100;
        let (cmd_tx, cmd_rx) = unbounded();
        let snapshot = Arc::new(Mutex::new(LooperSnapshot {
            tracks: Vec::new(),
            active_track_idx: 0,
            master_track_idx: None,
            is_master_playing: true,
            mic_peak_level: 0.0,
            monitor_mic: false,
            monitor_volume: 1.0,
            input_gain: 1.0,
            sample_rate,
            metro_enabled: false,
            metro_bpm: 120.0,
            metro_volume: 0.7,
            metro_cue_only: true,
            metro_beat: 0,
        }));

        let mut processor = LooperProcessor::new(sample_rate, cmd_rx, snapshot);

        // Trigger Track 0 to record (Master)
        cmd_tx.send(AudioCommand::TriggerTrack(0)).unwrap();

        // Process a block of 500 frames of input
        let input_block = vec![(0.6, 0.6); 500];
        let mut output_block = vec![(0.0, 0.0); 500];
        processor.process_block(&input_block, &mut output_block);

        assert_eq!(processor.tracks[0].state, TrackState::Recording);
        assert_eq!(processor.master_track_idx, Some(0));

        // Stop recording Track 0 -> transitions to Playing
        cmd_tx.send(AudioCommand::TriggerTrack(0)).unwrap();
        processor.process_block(&input_block, &mut output_block);
        assert_eq!(processor.tracks[0].state, TrackState::Playing);
        assert_eq!(processor.tracks[0].length_frames, 500);

        // Trigger Track 1 to record -> should enter QueuedRecord to wait for master loop start
        cmd_tx.send(AudioCommand::TriggerTrack(1)).unwrap();
        processor.process_block(&input_block[0..10], &mut output_block[0..10]);
        // While master hasn't wrapped, Track 1 is queued
        assert_eq!(processor.tracks[1].state, TrackState::QueuedRecord);
    }

    #[test]
    fn test_mpc_modes_playback() {
        use crate::audio::types::PlaybackMode;
        let mut track = LoopTrack::new(0, 44100);
        track.start_recording();
        // Record 100 frames
        for _ in 0..100 {
            track.record_frame(0.8, 0.8);
        }
        track.finish_recording();
        assert_eq!(track.length_frames, 100);
        assert_eq!(track.state, TrackState::Playing);

        // Test: Mpc (One-Shot) plays to end and stops (is_paused = true)
        track.mode = PlaybackMode::Mpc;
        track.playhead = 0;
        track.is_paused = false;

        for _ in 0..100 {
            let (_, _, _) = track.read_and_advance(false);
        }
        // At frame 100, playhead wrapped and Mpc stops the track
        assert!(track.is_paused);
        // Advancing while paused yields silent 0.0
        let (l, r, _) = track.read_and_advance(false);
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);

        // Retrigger resets playhead and unpauses
        track.playhead = 0;
        track.is_paused = false;
        let (l2, r2, _) = track.read_and_advance(false);
        assert!(l2 > 0.0 && r2 > 0.0);
    }
}
