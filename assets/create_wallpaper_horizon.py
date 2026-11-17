from PIL import Image, ImageDraw
import math

TO_RAD = math.pi / 180

class ArtificialHorizonWallpaper():
    def __init__(self, dims):
        self.__dict__.update(dims)
        #self.img = Image.new(mode='1', size=(self.width, self.height), color=1)
        self.img = Image.new(mode='P', size=(self.width, self.height), color=0)
        self.img.putpalette(b"\xff\xff\xff\x00\x00\x00\x80\x80\x80\x10\x10\x10")

        self.draw = ImageDraw.Draw(self.img)
        self.radius = self.width // 2
        self.center_x = self.radius
        self.center_y = self.radius
        self.ah_y_ofs = 0

    def level_tube(self):
        h = self.height / 5
        self.draw.rectangle((0, self.height - h, self.width, self.height), fill=3)

        my = int(self.height * 0.25)
        w = self.width // 2
        len = int(self.height * 0.65)
        radius = int(self.width * 0.035)
        alpha = 0.0
        delta = math.pi*0.1/180

        for idx in range(165):
            self.draw.circle((w + len*math.sin(alpha), my + len*math.cos(alpha)), radius, fill=2)
            self.draw.circle((w - len*math.sin(alpha), my + len*math.cos(alpha)), radius, fill=2)
            alpha += delta

        d = 3
        self.draw.line((w - radius - d, self.height - h, w - radius - d, self.height), width=self.ah_stroke_width, fill=3)
        self.draw.line((w + radius + d, self.height - h, w + radius + d, self.height), width=self.ah_stroke_width, fill=3)



    def ah_wallpaper(self):
        y_ofs = self.ah_y_ofs
        if self.show_arc_limited:
            self.draw.arc((0, y_ofs, self.width, self.width + y_ofs), 210, 330, width=self.ah_stroke_width, fill=1)
        else:
            self.draw.arc((0, y_ofs, self.width, self.width + y_ofs), 0, 360, width=self.ah_stroke_width, fill=1)

        r1 = self.radius - self.ah_stroke_len
        r2 = self.radius
        cx = self.center_x
        cy = self.ah_y_ofs + self.width // 2
        delta = math.pi*15/180
        alpha = -math.pi*240/180

        for idx in range(9):
            self.draw.line(
                (
                    cx + int(r1*math.sin(alpha)), 
                    cy + int(r1*math.cos(alpha)),
                    cx + int(r2*math.sin(alpha)), 
                    cy + int(r2*math.cos(alpha))
                ),
                width=self.ah_stroke_width,
                fill=1
            )
            alpha += delta

        w = self.ah_point_width
        self.draw.arc((cx - w, cy - w, cx + w, cy + w), 0, 360, width=10, fill=1)

        x1 = int(self.radius*0.2)
        x2 = int(self.radius*0.8)
        x3 = int(self.radius*0.05)
        x4 = x1 + self.ah_glider_width / 2
        y1 = int(self.radius*0.15)
        self.draw.line((cx - x1, cy, cx - x2 - 1, cy), width=self.ah_glider_width, fill=1)
        self.draw.line((cx + x1, cy, cx + x2 + 1, cy), width=self.ah_glider_width, fill=1)

        self.draw.line((cx - x2, cy + 1, cx - x2 - x3, cy - y1), width=self.ah_glider_width, fill=1)
        self.draw.line((cx + x2, cy + 1, cx + x2 + x3, cy - y1), width=self.ah_glider_width, fill=1)


    def generate(self, path):
        self.ah_wallpaper()
        self.level_tube()
        self.img.save(path)
        print(f"  Save as '{path}'")
    
    def show(self):
        self.img.show()



DIMS_480_480 = {
    "show_arc_limited": False,
    "width": 480,
    "height": 480,
    "bottom_line": 430,
    "ah_stroke_len": 27,
    "ah_stroke_width": 3,
    "ah_point_width": 7,
    "ah_glider_width": 6,
    "comp_stroke_len": -27,
    "comp_stroke_width": 3,
}

print("create_wallpaper_horizon.py")
wp = ArtificialHorizonWallpaper(DIMS_480_480)
wp.generate("assets/size_480x480/wp_horizon.png")


class RingHorizonWallpaper(ArtificialHorizonWallpaper):
    """Horizon wallpaper for the inner circle of the ring vario page.

    Exactly the same drawing as the full screen version, scaled by
    ring_radius / (width/2) around the centre of the display and clipped to
    the circle. So the small horizon shows the very same lines: outer ring,
    roll scale, centre point, glider symbol and the level tube.
    """

    def __init__(self, dims):
        super().__init__(dims)
        self.s = self.ring_radius / (self.width / 2)

    def _scale(self, v):
        return v * self.s

    def _pt(self, x, y):
        """scale a full screen coordinate towards the display centre"""
        cx = self.width / 2
        cy = self.width / 2
        return (cx + (x - cx) * self.s, cy + (y - cy) * self.s)

    # The outer ring is the border between the sky/earth fill of the
    # firmware and the black background. It is therefore not drawn with
    # PIL's arc rasterizer but pixel exact, so that it is a perfect circle
    # and the firmware fill (radius RING_FILL) always ends underneath it.
    RING_OUTER = 0.5   # outer edge:  d <= ring_radius + RING_OUTER
    RING_INNER = 2.5   # inner edge:  d >= ring_radius - RING_INNER (3 px, as big)
    RING_FILL = 1      # radius the firmware fills sky and earth with

    def _annulus(self, r_out, r_in, color):
        """exact ring by pixel distance - no rasterizer, no steps"""
        cx = self.width / 2
        cy = self.width / 2
        px = self.img.load()
        y0 = max(0, int(cy - r_out) - 1)
        y1 = min(self.height - 1, int(cy + r_out) + 1)
        o2 = r_out * r_out
        i2 = r_in * r_in
        for y in range(y0, y1 + 1):
            dy = y - cy
            for x in range(self.width):
                dx = x - cx
                d2 = dx * dx + dy * dy
                if i2 <= d2 <= o2:
                    px[x, y] = color

    def ah_wallpaper(self):
        cx = self.width / 2
        cy = self.width / 2
        r = self.ring_radius

        # outer ring - pixel exact, thick enough to cover the fill edge
        self._annulus(r + self.RING_OUTER, r - self.RING_INNER, 1)

        # roll scale: nine ticks every 15 degrees, same as the big one
        w = max(2, int(round(self.ah_stroke_width * self.s)))
        r1 = r - self.ah_stroke_len * self.s
        delta = math.pi * 15 / 180
        alpha = -math.pi * 240 / 180
        for idx in range(9):
            self.draw.line(
                (
                    cx + int(r1 * math.sin(alpha)),
                    cy + int(r1 * math.cos(alpha)),
                    cx + int(r * math.sin(alpha)),
                    cy + int(r * math.cos(alpha)),
                ),
                width=w,
                fill=1,
            )
            alpha += delta

        # centre point
        pw = self.ah_point_width * self.s
        self.draw.arc((cx - pw, cy - pw, cx + pw, cy + pw), 0, 360,
                      width=max(3, int(10 * self.s)), fill=1)

        # glider symbol
        gw = max(3, int(round(self.ah_glider_width * self.s)))
        x1 = int(r * 0.2)
        x2 = int(r * 0.8)
        x3 = int(r * 0.05)
        y1 = int(r * 0.15)
        self.draw.line((cx - x1, cy, cx - x2 - 1, cy), width=gw, fill=1)
        self.draw.line((cx + x1, cy, cx + x2 + 1, cy), width=gw, fill=1)
        self.draw.line((cx - x2, cy + 1, cx - x2 - x3, cy - y1), width=gw, fill=1)
        self.draw.line((cx + x2, cy + 1, cx + x2 + x3, cy - y1), width=gw, fill=1)

    def level_tube(self):
        cx = self.width / 2
        # same geometry as the big level tube, scaled around the centre
        my = self._pt(0, int(self.height * 0.25))[1]
        length = self._scale(int(self.height * 0.65))
        radius = self._scale(int(self.width * 0.035))
        band_top = self._pt(0, self.height - self.height / 5)[1]
        band_bottom = self._pt(0, self.height)[1]

        self.draw.rectangle((0, band_top, self.width, band_bottom), fill=3)

        alpha = 0.0
        delta = math.pi * 0.1 / 180
        for idx in range(165):
            self.draw.circle((cx + length * math.sin(alpha), my + length * math.cos(alpha)),
                             radius, fill=2)
            self.draw.circle((cx - length * math.sin(alpha), my + length * math.cos(alpha)),
                             radius, fill=2)
            alpha += delta

        d = self._scale(3)
        lw = max(2, int(round(self.ah_stroke_width * self.s)))
        self.draw.line((cx - radius - d, band_top, cx - radius - d, band_bottom),
                       width=lw, fill=3)
        self.draw.line((cx + radius + d, band_top, cx + radius + d, band_bottom),
                       width=lw, fill=3)

    def clip_to_circle(self):
        """everything outside the circle becomes transparent (index 0)"""
        cx = self.width / 2
        cy = self.width / 2
        r = self.ring_radius + self.RING_OUTER
        px = self.img.load()
        for y in range(self.height):
            dy = y - cy
            for x in range(self.width):
                dx = x - cx
                if dx * dx + dy * dy > r * r:
                    px[x, y] = 0

    def generate(self, path):
        self.ah_wallpaper()
        self.level_tube()
        self.clip_to_circle()
        self.img.save(path)
        print(f"  Save as '{path}'")


DIMS_RING_480 = {
    "show_arc_limited": False,
    "width": 480,
    "height": 480,
    "bottom_line": 430,
    "ah_stroke_len": 27,
    "ah_stroke_width": 3,
    "ah_point_width": 7,
    "ah_glider_width": 6,
    "comp_stroke_len": -27,
    "comp_stroke_width": 3,
    # inner circle of the ring vario page (height * 5 / 16)
    "ring_radius": 150,
}

wp = RingHorizonWallpaper(DIMS_RING_480)
wp.generate("assets/size_480x480/wp_horizon_ring.png")
