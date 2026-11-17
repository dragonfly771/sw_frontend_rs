from PIL import Image, ImageDraw, ImageFont
import math

TO_RAD = math.pi / 180

print("create_wallpaper_vario.py")


class Ring360Wallpaper():
    """Full-circle scale of the ring design (Stefly design), v2.

    Zero at the 9 o'clock position, positive values run clockwise over the
    top, plus and minus full scale meet at the 3 o'clock position
    (36 deg per unit at full scale 5).

    Geometry (480x480 reference, variant B3 of 27.10.2026 - larger core):
      scale ring circle r=202 w2.5 | center circle r=78 w2.5
      major ticks r202->209 w6 plus rounded cap (outer edge r212)
      minor ticks r202->212 w3
      numbers OUTSIDE the ticks at r=225, mono bold, digit height 20px
    Until 0.4.1.33: ring r=181, major ticks w8 to r200, minor w3.5 to r193,
    numbers r=221 / 21px (dark core of the light band r=168 -> now r=192).

    Rendered supersampled in grayscale, then thresholded to 1 bit
    (pixel value 0 = scale color, 255 = background - see convert_pictures).
    """
    SS = 4  # supersampling factor

    def __init__(self, dims):
        self.__dict__.update(dims)
        ss = self.SS
        self.img = Image.new(mode='L', size=(self.width * ss, self.height * ss), color=0)
        self.draw = ImageDraw.Draw(self.img)
        self.cx = self.width / 2
        self.cy = self.height / 2
        try:
            font_path = "assets/DejaVuSansMono-Bold.ttf"
            self.font = ImageFont.truetype(font_path, 8)
        except OSError:
            font_path = "assets/Arial_Bold.ttf"
        # calibrate the font size so digits are number_height px high
        for size in range(8, 60 * ss):
            f = ImageFont.truetype(font_path, size)
            bb = f.getbbox("0123456789")
            if bb[3] - bb[1] >= self.number_height * ss:
                self.font = f
                break

    def _polar(self, angle_deg, radius):
        a = angle_deg * TO_RAD
        ss = self.SS
        return ((self.cx - math.cos(a) * radius) * ss, (self.cy - math.sin(a) * radius) * ss)

    def tick(self, angle_deg, r_from, r_to, width, rounded_cap=False):
        ss = self.SS
        p1 = self._polar(angle_deg, r_from)
        p2 = self._polar(angle_deg, r_to)
        self.draw.line((*p1, *p2), fill=255, width=round(width * ss))
        if rounded_cap:
            r = width / 2 * ss
            self.draw.ellipse((p2[0] - r, p2[1] - r, p2[0] + r, p2[1] + r), fill=255)

    def number(self, value, angle_deg, label=None):
        x, y = self._polar(angle_deg, self.number_radius)
        text = str(abs(value)) if label is None else str(label)
        self.draw.text((x, y), text, font=self.font, fill=255, anchor="mm")

    def circle(self, radius, width):
        ss = self.SS
        self.draw.ellipse(
            ((self.cx - radius) * ss, (self.cy - radius) * ss,
             (self.cx + radius) * ss, (self.cy + radius) * ss),
            outline=255, width=round(width * ss))

    def generate(self, path, is_ten=False, overrange=False):
        """overrange: the numbers of the negative half continue the positive
        scale (6..9 resp. 12..18) - shown while the needle swings beyond full
        scale into the negative half, so the reading stays unambiguous"""
        self.circle(self.ring_radius, self.circle_width)
        self.circle(self.center_circle, self.circle_width)

        full = 10 if is_ten else 5
        for i in range(-10, 11):
            ang = i * 18.0
            if i % 2 == 0:  # major tick with rounded outer cap
                self.tick(ang, self.ring_radius, self.major_to - self.major_width / 2,
                          self.major_width, rounded_cap=True)
                value = i if is_ten else i // 2
                label = 2 * full - abs(value) if (overrange and value < 0) else None
                self.number(value, ang, label)
            else:  # minor tick
                self.tick(ang, self.ring_radius, self.minor_to, self.minor_width)

        # downscale, threshold and invert: marks -> 0, background -> 255
        small = self.img.resize((self.width, self.height), Image.LANCZOS)
        out = small.point(lambda v: 0 if v > 150 else 255).convert('1')
        out.save(path)
        print(f"  Save as '{path}'")


DIMS_RING_480_480 = {
    "width": 480,
    "height": 480,
    "ring_radius": 202,
    "center_circle": 78,
    "circle_width": 2.5,
    "major_to": 212,      # outer edge incl. rounded cap
    "major_width": 6,
    "minor_to": 212,
    "minor_width": 3,
    "number_radius": 225,
    "number_height": 20,
}

wp = Ring360Wallpaper(DIMS_RING_480_480)
wp.generate("assets/size_480x480/wp_vario360-5.png")
wp = Ring360Wallpaper(DIMS_RING_480_480)
wp.generate("assets/size_480x480/wp_vario360-10.png", is_ten=True)
# overrange variants (needle beyond full scale): 6..9 resp. 12..18 on the negative half
wp = Ring360Wallpaper(DIMS_RING_480_480)
wp.generate("assets/size_480x480/wp_vario360-5x.png", overrange=True)
wp = Ring360Wallpaper(DIMS_RING_480_480)
wp.generate("assets/size_480x480/wp_vario360-10x.png", is_ten=True, overrange=True)
