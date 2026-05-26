#!/usr/bin/env python3
"""
Starship telemetry stub for rosa demo.

Publishes plausible /starship/* telemetry topics and reacts to commands.
No Isaac Sim needed — this is the pure-ROS 2 fallback for the demo.

Run inside the ROS 2 container:
    docker cp examples/starship/stub.py rosa-ros2:/starship_stub.py
    docker exec -it rosa-ros2 bash -ic "python3 /starship_stub.py"

Physics model (simple, illustrative):
    hover_throttle = 0.45  (thrust exactly cancels gravity at this setting)
    accel_z = (throttle - hover_throttle) * 20  m/s^2
    altitude += accel_z * dt            (clamped at 0)
    fuel_burn = throttle * 0.0001 / tick (full throttle empties in ~2.7 hours sim time)
"""

import json
import sys
import time

import rclpy
from rclpy.node import Node
from std_msgs.msg import Bool, Float32, Float64, String
from geometry_msgs.msg import PoseStamped, Vector3, Vector3Stamped
from sensor_msgs.msg import Imu

HOVER_THROTTLE = 0.45   # throttle that cancels gravity
ACCEL_SCALE    = 20.0   # m/s^2 per unit of (throttle - hover)
FUEL_BURN_RATE = 0.0001 # fraction burned per 0.1 s tick at throttle=1.0
DT             = 0.1    # seconds per physics tick


class StarshipStub(Node):
    def __init__(self):
        super().__init__('starship_stub')

        # ── vehicle state ─────────────────────────────────────────────────
        self.altitude_m     = 0.0      # metres above ground
        self.velocity_z     = 0.0      # m/s vertical
        self.fuel_fraction  = 0.80     # 0.0–1.0
        self.throttle       = 0.0      # 0.0–0.85
        self.gimbal_pitch   = 0.0      # rad
        self.gimbal_yaw     = 0.0      # rad
        self.safe_mode      = False
        self.engine_state   = "idle"   # idle | nominal | safe_mode

        # ── publishers ───────────────────────────────────────────────────
        qos = 10
        self.pub_alt    = self.create_publisher(Float64,         '/starship/altitude',      qos)
        self.pub_fuel   = self.create_publisher(Float32,         '/starship/fuel_fraction', qos)
        self.pub_engine = self.create_publisher(String,          '/starship/engine_state',  qos)
        self.pub_vel    = self.create_publisher(Vector3Stamped,  '/starship/velocity',      qos)
        self.pub_pose   = self.create_publisher(PoseStamped,     '/starship/pose',          qos)

        # ── subscribers ──────────────────────────────────────────────────
        self.create_subscription(Float32,   '/starship/main_throttle', self._cb_throttle,  qos)
        self.create_subscription(Vector3,   '/starship/main_gimbal',   self._cb_gimbal,    qos)
        self.create_subscription(Bool,      '/starship/safe_mode',     self._cb_safe_mode, qos)

        # ── physics + publish timer ───────────────────────────────────────
        self.create_timer(DT, self._tick)

        self.get_logger().info(
            'Starship stub running — publishing telemetry on /starship/*\n'
            '  altitude=0 m  fuel=80%  engine=idle\n'
            '  Listening on /starship/main_throttle, /starship/main_gimbal, /starship/safe_mode'
        )

    # ── command callbacks ──────────────────────────────────────────────────

    def _cb_throttle(self, msg: Float32):
        if self.safe_mode:
            self.get_logger().warn('Safe mode engaged — throttle command ignored')
            return
        self.throttle = max(0.0, min(0.85, float(msg.data)))
        self.engine_state = 'nominal' if self.throttle > 0.01 else 'idle'
        self.get_logger().info(f'throttle → {self.throttle:.3f}  state={self.engine_state}')

    def _cb_gimbal(self, msg: Vector3):
        self.gimbal_pitch = max(-0.26, min(0.26, float(msg.x)))
        self.gimbal_yaw   = max(-0.26, min(0.26, float(msg.y)))
        self.get_logger().info(f'gimbal pitch={self.gimbal_pitch:.3f} yaw={self.gimbal_yaw:.3f}')

    def _cb_safe_mode(self, msg: Bool):
        self.safe_mode = bool(msg.data)
        if self.safe_mode:
            self.throttle     = 0.0
            self.engine_state = 'safe_mode'
        self.get_logger().warn(f'safe_mode → {self.safe_mode}')

    # ── physics tick ───────────────────────────────────────────────────────

    def _tick(self):
        # vertical acceleration from throttle vs gravity
        accel_z = (self.throttle - HOVER_THROTTLE) * ACCEL_SCALE
        self.velocity_z  = accel_z  # simplified: no velocity accumulation for clarity
        self.altitude_m  = max(0.0, self.altitude_m + accel_z * DT)

        # fuel consumption
        if self.throttle > 0.0:
            self.fuel_fraction = max(0.0, self.fuel_fraction - self.throttle * FUEL_BURN_RATE)

        # publish
        now = self.get_clock().now().to_msg()

        alt_msg = Float64(); alt_msg.data = self.altitude_m
        self.pub_alt.publish(alt_msg)

        fuel_msg = Float32(); fuel_msg.data = float(self.fuel_fraction)
        self.pub_fuel.publish(fuel_msg)

        engine_json = json.dumps({
            'state':    self.engine_state,
            'throttle': round(self.throttle, 3),
            'gimbal':   {'pitch': round(self.gimbal_pitch, 4), 'yaw': round(self.gimbal_yaw, 4)},
            'temp_K':   3200 if self.throttle > 0.0 else 300,
        })
        engine_msg = String(); engine_msg.data = engine_json
        self.pub_engine.publish(engine_msg)

        vel_msg = Vector3Stamped()
        vel_msg.header.stamp = now
        vel_msg.header.frame_id = 'world'
        vel_msg.vector.z = self.velocity_z
        self.pub_vel.publish(vel_msg)

        pose_msg = PoseStamped()
        pose_msg.header.stamp = now
        pose_msg.header.frame_id = 'world'
        pose_msg.pose.position.z = self.altitude_m
        pose_msg.pose.orientation.w = 1.0
        self.pub_pose.publish(pose_msg)


def main():
    rclpy.init(args=sys.argv)
    node = StarshipStub()
    try:
        rclpy.spin(node)
    except KeyboardInterrupt:
        node.get_logger().info('Stub shutting down.')
    finally:
        node.destroy_node()
        rclpy.shutdown()


if __name__ == '__main__':
    main()
