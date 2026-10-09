package org.civ3touch.spike;

public final class MapCameraTest {
    private static void close(float actual, float expected) {
        if (Math.abs(actual - expected) > .001) throw new AssertionError(actual + " != " + expected);
    }
    public static void main(String[] args) {
        MapCamera camera = new MapCamera();
        camera.center(8, 8);
        camera.pan(96, 0, 48, 28, 16, 16);
        close(camera.x, 7); close(camera.y, 9);
        camera.pan(-96, 0, 48, 28, 16, 16);
        close(camera.x, 8); close(camera.y, 8);
        // The world point beneath an off-centre pinch remains beneath that focus.
        camera.scale(2, 48, 28, 48, 28, 16, 16);
        close(camera.x + (48f / 96 + 28f / 56) / 2, 9);
        close(camera.y + (28f / 56 - 48f / 96) / 2, 8);
        close(camera.zoom, 2);
        camera.scale(100, 0, 0, 96, 56, 16, 16); close(camera.zoom, 2);
        camera.scale(.001f, 0, 0, 96, 56, 16, 16); close(camera.zoom, .65f);
        camera.pan(100000, 100000, 48, 28, 16, 16);
        close(camera.x, 0); close(camera.y, 0);
        camera.pan(-100000, -100000, 48, 28, 16, 16);
        close(camera.x, 15); close(camera.y, 15);
        System.out.println("PASS: map pan, pinch focus, zoom limits and world bounds");
    }
}
