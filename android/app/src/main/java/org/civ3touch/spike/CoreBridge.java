package org.civ3touch.spike;

final class CoreBridge {
    static { System.loadLibrary("civ3touch_core"); }
    private CoreBridge() {}
    static native String newGame(String rulesDirectory);
    static native String moveUnit(int index, int generation, int x, int y);
    static native String endTurn();
    static native void close();
    static native int smokeTest(String rulesDirectory);
}
