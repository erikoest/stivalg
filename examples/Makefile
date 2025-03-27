SV := ../target/release/stivalg

gpx:= $(patsubst %.json,%.gpx,$(wildcard *.json))

all: $(gpx)

clean:
	rm -f *.gpx *~

%.gpx: %.json
	$(SV) -c $< --output $@

.PHONY: all clean
