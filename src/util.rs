pub fn wrap_degrees(mut angle: f32) -> f32 {
    angle %= 360.0;
    if angle >= 180.0 {
        angle -= 360.0;
    }
    if angle < -180.0 {
        angle += 360.0;
    }
    angle
}
