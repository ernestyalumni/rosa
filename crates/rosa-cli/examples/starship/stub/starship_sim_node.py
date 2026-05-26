#!/usr/bin/env python3
"""
starship_sim_node.py — Lightweight Starship physics stub for ROS 2.

Runs WITHOUT Isaac Sim.  Simulates simplified 1-D vertical dynamics
(pitch/yaw gimbal + RCS produce angular impulses, not full 6-DOF here).

Publishes:
  /starship/pose           geometry_msgs/PoseStamped   ~50 Hz
  /starship/altitude       std_msgs/Float64             ~50 Hz  (metres AGL)
  /starship/velocity       geometry_msgs/Vector3Stamped ~50 Hz
  /starship/imu            sensor_msgs/Imu              ~50 Hz
  /starship/fuel_fraction  std_msgs/Float32              ~1 Hz
  /starship/engine_state   std_msgs/String              ~10 Hz

Subscribes:
  /starship/main_throttle  std_msgs/Float32
  /starship/main_gimbal    geometry_msgs/Vector3
  /starship/rcs/top        geometry_msgs/Vector3
  /starship/rcs/mid_fwd    geometry_msgs/Vector3
  /starship/rcs/mid_aft    geometry_msgs/Vector3
  /starship/safe_mode      std_msgs/Bool

Services:
  /starship/reset          std_srvs/Empty — reset to launch pad

Usage:
  # Inside the rosa-ros2 container:
  docker exec -it rosa-ros2 bash -ic "python3 /starship_sim_node.py"

  # Or from host (requires ROS 2 setup):
  ros2 run --package <n/a> python3 starship_sim_node.py
"""

import json
import math
import sys
import threading
import time

import rclpy
from rclpy.node import Node
from builtin_interfaces.msg import Time as RosTime
from geometry_msgs.msg import PoseStamped, Vector3, Vector3Stamped
from sensor_msgs.msg import Imu
from std_msgs.msg import Bool, Float32, Float64, String
from std_srvs.srv import Empty

# ── Physics constants ─────────────────────────────────────────────────────
# Calibrated so hover throttle ≈ 0.45–0.55 depending on propellant load,
# matching the FSW system-prompt spec (0.0 = empty, 1.0 = full tank).
#   Full-fuel (830 t total) hover throttle: 830k×9.81/14.7M ≈ 0.554
#   Half-fuel (480 t total) hover throttle: 480k×9.81/14.7M ≈ 0.321
G            = 9.81          # m/s²
DRY_MASS     = 130_000.0     # kg  (upper-stage dry mass)
MAX_THRUST   = 14_700_000.0  # N   (6× Raptor Vacuum, simplified)
MAX_THROTTLE = 0.85
FUEL_CAP     = 700_000.0     # kg  (propellant; gives ≈55% hover at full tank)
BURN_RATE    = 2_000.0       # kg/s at 100% throttle (≈6 min burn to empty)
MOMENT_I     = 5e9           # kg·m²  (roll/pitch)
RCS_TORQUE   = 5e6           # N·m per unit impulse


class StarshipSimNode(Node):
    def __init__(self) -> None:
        super().__init__("starship_sim_node")

        # ── State ─────────────────────────────────────────────────────────
        self._lock       = threading.Lock()
        self._altitude   = 0.0     # m AGL
        self._vel_y      = 0.0     # m/s vertical
        self._pitch      = 0.0     # rad (world frame tilt)
        self._yaw        = 0.0     # rad
        self._dpitch     = 0.0     # rad/s
        self._dyaw       = 0.0     # rad/s
        self._fuel       = FUEL_CAP
        self._throttle   = 0.0
        self._gimbal_pitch = 0.0
        self._gimbal_yaw   = 0.0
        self._safe_mode  = False

        # ── Publishers ────────────────────────────────────────────────────
        self._pub_pose   = self.create_publisher(PoseStamped,   "/starship/pose",         10)
        self._pub_alt    = self.create_publisher(Float64,        "/starship/altitude",     10)
        self._pub_vel    = self.create_publisher(Vector3Stamped, "/starship/velocity",     10)
        self._pub_imu    = self.create_publisher(Imu,            "/starship/imu",          10)
        self._pub_fuel   = self.create_publisher(Float32,        "/starship/fuel_fraction", 10)
        self._pub_eng    = self.create_publisher(String,         "/starship/engine_state", 10)

        # ── Subscribers ───────────────────────────────────────────────────
        self.create_subscription(Float32, "/starship/main_throttle", self._cb_throttle, 10)
        self.create_subscription(Vector3, "/starship/main_gimbal",   self._cb_gimbal,   10)
        self.create_subscription(Vector3, "/starship/rcs/top",       self._cb_rcs_top,  10)
        self.create_subscription(Vector3, "/starship/rcs/mid_fwd",   self._cb_rcs_mf,   10)
        self.create_subscription(Vector3, "/starship/rcs/mid_aft",   self._cb_rcs_ma,   10)
        self.create_subscription(Bool,    "/starship/safe_mode",     self._cb_safe,     10)

        # ── Services ──────────────────────────────────────────────────────
        self.create_service(Empty, "/starship/reset", self._srv_reset)

        # ── Timers ────────────────────────────────────────────────────────
        dt = 0.02   # 50 Hz physics + telemetry
        self._last_t = time.monotonic()
        self._tick_count = 0
        self.create_timer(dt, self._tick)

        self.get_logger().info("Starship sim node started — altitude=0 m, fuel=100%")

    # ── Callbacks ─────────────────────────────────────────────────────────

    def _cb_throttle(self, msg: Float32) -> None:
        with self._lock:
            self._throttle = max(0.0, min(MAX_THROTTLE, float(msg.data)))

    def _cb_gimbal(self, msg: Vector3) -> None:
        with self._lock:
            self._gimbal_pitch = max(-0.26, min(0.26, float(msg.x)))
            self._gimbal_yaw   = max(-0.26, min(0.26, float(msg.y)))

    def _cb_rcs_top(self, msg: Vector3) -> None:
        with self._lock:
            torque = float(msg.z) * RCS_TORQUE   # z → yaw torque
            pitch_t = float(msg.x) * RCS_TORQUE
            self._dpitch += pitch_t / MOMENT_I * 0.02
            self._dyaw   += torque  / MOMENT_I * 0.02

    def _cb_rcs_mf(self, msg: Vector3) -> None:
        with self._lock:
            self._dpitch += float(msg.x) * RCS_TORQUE / MOMENT_I * 0.02

    def _cb_rcs_ma(self, msg: Vector3) -> None:
        with self._lock:
            self._dpitch -= float(msg.x) * RCS_TORQUE / MOMENT_I * 0.02

    def _cb_safe(self, msg: Bool) -> None:
        with self._lock:
            self._safe_mode = bool(msg.data)
            if self._safe_mode:
                self._throttle     = 0.0
                self._gimbal_pitch = 0.0
                self._gimbal_yaw   = 0.0
        mode = "ENGAGED" if msg.data else "disengaged"
        self.get_logger().info(f"safe_mode {mode}")

    def _srv_reset(self, _req, response):
        with self._lock:
            self._altitude   = 0.0
            self._vel_y      = 0.0
            self._pitch      = 0.0
            self._yaw        = 0.0
            self._dpitch     = 0.0
            self._dyaw       = 0.0
            self._fuel       = FUEL_CAP
            self._throttle   = 0.0
            self._gimbal_pitch = 0.0
            self._gimbal_yaw   = 0.0
            self._safe_mode  = False
        self.get_logger().info("Starship reset to launch pad")
        return response

    # ── Physics step + publish ─────────────────────────────────────────────

    def _tick(self) -> None:
        now = time.monotonic()
        dt  = now - self._last_t
        self._last_t = now
        self._tick_count += 1

        with self._lock:
            throttle     = 0.0 if self._safe_mode else self._throttle
            gp           = self._gimbal_pitch
            gy           = self._gimbal_yaw
            safe         = self._safe_mode

            # ── Physics ───────────────────────────────────────────────────
            if self._fuel > 0.0 and throttle > 0.0:
                thrust = throttle * MAX_THRUST
                # Vertical component (main engine ≈ vertical for small gimbal)
                f_y = thrust * math.cos(gp) * math.cos(gy)
                # Fuel burn
                self._fuel = max(0.0, self._fuel - throttle * BURN_RATE * dt)
            else:
                f_y = 0.0
                if self._fuel <= 0.0:
                    throttle = 0.0  # no fuel → no thrust

            net_mass = DRY_MASS + self._fuel
            accel_y  = (f_y / net_mass) - G

            # Attitude dynamics (gimbal torque)
            torque_pitch = f_y * 5.0 * math.sin(gp)   # moment_arm = 5 m
            torque_yaw   = f_y * 5.0 * math.sin(gy)
            self._dpitch += (torque_pitch / MOMENT_I) * dt
            self._dyaw   += (torque_yaw   / MOMENT_I) * dt

            # Integrate
            self._vel_y   += accel_y      * dt
            self._altitude = max(0.0, self._altitude + self._vel_y * dt)
            self._pitch   += self._dpitch * dt
            self._yaw     += self._dyaw   * dt

            # Ground contact: stop falling below 0
            if self._altitude <= 0.0:
                self._altitude = 0.0
                if self._vel_y < 0.0:
                    self._vel_y  = 0.0
                    self._dpitch = 0.0
                    self._dyaw   = 0.0

            # Snapshot for publishing
            alt   = self._altitude
            vel_y = self._vel_y
            pitch = self._pitch
            yaw   = self._yaw
            dpitch = self._dpitch
            dyaw   = self._dyaw
            fuel  = self._fuel
            accel = accel_y

        # ── Stamps ────────────────────────────────────────────────────────
        t = self.get_clock().now()
        sec, nanosec = divmod(t.nanoseconds, 1_000_000_000)
        stamp = RosTime()
        stamp.sec     = int(sec)
        stamp.nanosec = int(nanosec)

        # ── /starship/altitude + /starship/velocity ───────────────────────
        alt_msg = Float64(); alt_msg.data = alt
        self._pub_alt.publish(alt_msg)

        vel_msg = Vector3Stamped()
        vel_msg.header.stamp    = stamp
        vel_msg.header.frame_id = "world"
        vel_msg.vector.y = vel_y
        self._pub_vel.publish(vel_msg)

        # ── /starship/pose ────────────────────────────────────────────────
        pose = PoseStamped()
        pose.header.stamp    = stamp
        pose.header.frame_id = "world"
        pose.pose.position.y = alt
        # Euler → quaternion (pitch-yaw only, no roll)
        cp, sp = math.cos(pitch / 2), math.sin(pitch / 2)
        cy, sy = math.cos(yaw   / 2), math.sin(yaw   / 2)
        pose.pose.orientation.x = sp * cy
        pose.pose.orientation.y = cp * sy
        pose.pose.orientation.z = sp * sy
        pose.pose.orientation.w = cp * cy
        self._pub_pose.publish(pose)

        # ── /starship/imu ─────────────────────────────────────────────────
        imu = Imu()
        imu.header.stamp    = stamp
        imu.header.frame_id = "starship/imu"
        imu.angular_velocity.x = dpitch
        imu.angular_velocity.z = dyaw
        imu.linear_acceleration.y = accel
        self._pub_imu.publish(imu)

        # ── /starship/fuel_fraction (1 Hz) ────────────────────────────────
        if self._tick_count % 50 == 0:
            fuel_msg = Float32()
            fuel_msg.data = float(fuel / FUEL_CAP)
            self._pub_fuel.publish(fuel_msg)

        # ── /starship/engine_state (10 Hz) ────────────────────────────────
        if self._tick_count % 5 == 0:
            state = {
                "throttle":     round(throttle, 4),
                "gimbal_pitch": round(gp, 4),
                "gimbal_yaw":   round(gy, 4),
                "safe_mode":    safe,
                "fuel_kg":      round(fuel, 1),
            }
            eng = String(); eng.data = json.dumps(state)
            self._pub_eng.publish(eng)


def main() -> None:
    rclpy.init()
    node = StarshipSimNode()
    try:
        rclpy.spin(node)
    except KeyboardInterrupt:
        pass
    finally:
        node.destroy_node()
        rclpy.shutdown()


if __name__ == "__main__":
    main()
