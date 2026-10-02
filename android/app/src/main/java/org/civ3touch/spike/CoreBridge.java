package org.civ3touch.spike;

final class CoreBridge {
    static { System.loadLibrary("civ3touch_core"); }
    private CoreBridge() {}
    static native int smokeTest(String rulesDirectory);
}
