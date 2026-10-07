package org.civ3touch.spike;

final class CoreBridge {
    static { System.loadLibrary("civ3touch_core"); }
    private CoreBridge() {}
    static native String importProfile();
    static native String loadAssets(String directory);
    static native int[] imagePixels(String key);
    static native String newGame(String rulesDirectory);
    static native String moveUnit(int index, int generation, int x, int y);
    static native String endTurn();
    static native String command(String request);
    static native String snapshot();
    static native String save();
    static native String load(String rulesDirectory, String saved);
    static native void close();
    static native int smokeTest(String rulesDirectory);
}
