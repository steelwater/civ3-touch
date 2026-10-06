package org.civ3touch.spike;

import android.media.AudioAttributes;
import android.media.MediaPlayer;
import android.media.SoundPool;
import android.util.Log;

/** Main-thread playback, active only while the game is foreground and audio is enabled. */
final class GameAudio {
    private MediaPlayer music;
    private SoundPool effects;
    private int step;
    private boolean prepared, stepReady, playing;
    private final java.util.function.Consumer<String> failure;
    GameAudio(java.util.function.Consumer<String> failure) { this.failure = failure; }

    void load(ImportedAssets assets) {
        close();
        AudioAttributes attributes = new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_GAME)
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build();
        try {
            if (assets.music != null) {
                music = new MediaPlayer(); music.setAudioAttributes(attributes);
                music.setDataSource(assets.music.getPath()); music.setLooping(true); music.setVolume(.35f, .35f);
                music.setOnPreparedListener(player -> { prepared = true; evidence("MUSIC_READY"); if (playing) setPlaying(true); });
                music.setOnErrorListener((player, what, extra) -> { prepared = false; failure.accept("Music could not be played. Re-copy the optional MP3 and reimport."); return true; });
                music.prepareAsync();
            }
            if (assets.step != null) {
                effects = new SoundPool.Builder().setMaxStreams(1).setAudioAttributes(attributes).build();
                effects.setOnLoadCompleteListener((pool, sample, status) -> { stepReady = status == 0; if (stepReady) evidence("STEP_READY"); if (!stepReady) failure.accept("Movement sound could not be played. Re-copy the optional WAV and reimport."); });
                step = effects.load(assets.step.getPath(), 1);
            }
        } catch (Exception error) { close(); failure.accept("Optional audio could not be loaded. Re-copy audio and reimport; Play remains available."); }
    }
    void setPlaying(boolean active) {
        playing = active;
        if (music != null && prepared) {
            if (active && !music.isPlaying()) { music.start(); evidence("MUSIC_STARTED"); }
            else if (!active && music.isPlaying()) { music.pause(); evidence("MUSIC_PAUSED"); }
        }
        if (!active && effects != null) effects.autoPause();
    }
    void step() {
        if (playing && stepReady && effects != null && effects.play(step, .7f, .7f, 1, 0, 1) != 0) evidence("STEP_PLAYED");
    }
    private static void evidence(String event) { if (BuildConfig.DEBUG) Log.i("Civ3Touch", "AUDIO_" + event); }
    void close() {
        playing = false; prepared = false; stepReady = false;
        if (music != null) { music.release(); music = null; }
        if (effects != null) { effects.release(); effects = null; }
    }
}
